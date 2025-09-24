import os
import re
import shutil
import importlib
import sys
import zipfile
import json
import asyncio
import logging
import traceback
from pathlib import Path
from typing import Dict, Any, Optional, List, Union, Set, TypeVar, Callable, Tuple

from amia import Amia
from amia.recv_message import RecvMessage
from config import Config
from utools.match import recursive_match


# 定义项目接口，供插件调用
class ProjectInterface:
    """插件与主项目交互的接口。"""

    _instance: Optional["ProjectInterface"] = None
    bot: Amia

    def __new__(cls, *args, **kwargs) -> "ProjectInterface":
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance

    def __init__(self) -> None:
        """初始化项目接口实例。"""
        self.bot = Amia.get_instance()

    async def send_data_to_project(self, data: Dict[str, Any]) -> Dict[str, Any]:
        """
        示例：插件调用此方法向项目发送数据。

        Args:
            data (Dict[str, Any]): 要发送的数据。

        Returns:
            Dict[str, Any]: 响应字典。
        """
        logging.info(f"项目接收来自插件的数据: {json.dumps(data)}")
        # 这里可以实现特定的业务逻辑，如写入数据库或发送网络请求
        return {"status": "success", "message": "数据已接收"}


class Plugin:
    """表示一个插件，包含其元数据、状态和功能。"""

    def __init__(self, plugin_id: str, plugin_info: Dict[str, Any]) -> None:
        """
        使用插件的元数据和状态初始化Plugin实例。

        Args:
            plugin_id (str): 插件的唯一标识符。
            plugin_info (Dict[str, Any]): 包含插件元数据和状态的字典。
        """
        self.id: str = plugin_id
        self.name: str = plugin_info.get("name", plugin_id)
        self.description: str = plugin_info.get("description", "暂无描述")
        self.version: str = plugin_info.get("version", "1.0.0")
        self.author: str = plugin_info.get("author", "Unknown")
        self.triggers: List[Dict[str, Any]] = plugin_info.get("triggers", [])
        self.enabled: bool = plugin_info.get("enabled", False)
        self.loaded: bool = plugin_info.get("loaded", False)
        self.file_name: str = plugin_info.get("file_name", "")
        self.file_path: str = plugin_info.get("file_path", "")

        # 额外的插件元数据
        self.metadata: Dict[str, Any] = {
            k: v
            for k, v in plugin_info.items()
            if k
            not in [
                "id",
                "name",
                "description",
                "version",
                "author",
                "triggers",
                "enabled",
                "loaded",
                "file_name",
                "file_path",
            ]
        }

        # 加载的模块引用
        self.module: Any = None

    def to_dict(self) -> Dict[str, Any]:
        """
        将插件信息转换为字典。

        Returns:
            Dict[str, Any]: 包含所有插件信息的字典。
        """
        return {
            "id": self.id,
            "name": self.name,
            "description": self.description,
            "version": self.version,
            "author": self.author,
            "triggers": self.triggers,
            "enabled": self.enabled,
            "loaded": self.loaded,
            "file_name": self.file_name,
            "file_path": self.file_path,
            **self.metadata,
        }

    async def call_function(self, function_name: str, **kwargs: Any) -> Optional[Any]:
        """
        安全地调用插件模块中的函数。

        Args:
            function_name (str): 要调用的函数名称。
            **kwargs (Any): 传递给函数的关键字参数。

        Returns:
            Optional[Any]: 函数的返回值，如果发生错误则为None。
        """
        if not self.module or not self.loaded:
            logging.error(
                f"无法调用插件 '{self.id}' 中的函数 '{function_name}': 插件未加载。"
            )
            return None

        if not hasattr(self.module, function_name):
            logging.error(f"在插件 '{self.id}' 中未找到函数 '{function_name}'。")
            return None

        try:
            func = getattr(self.module, function_name)
            # 检查函数是否可等待并相应地调用
            if asyncio.iscoroutinefunction(func):
                return await func(**kwargs)
            else:
                return func(**kwargs)
        except Exception as e:
            logging.error(
                f"调用插件 '{self.id}' 中的函数 '{function_name}' 时出错: {e}\n"
                f"错误堆栈:\n{traceback.format_exc()}"
            )
            return None

    def is_triggered(self, trigger_type: str, trigger_data: Dict[str, Any]) -> bool:
        """
        检查插件是否被给定的触发器类型和数据触发。

        Args:
            trigger_type (str): 要检查的触发器类型。
            trigger_data (Dict[str, Any]): 与触发器关联的数据。

        Returns:
            bool: 如果插件被触发则为True，否则为False。
        """
        for trigger in self.triggers:
            if trigger.get("type") == trigger_type:
                # 这里可以实现实际的触发器匹配逻辑
                # 这是一个占位符实现
                return True
        return False


class PluginManager:
    """管理插件的生命周期，包括加载、卸载和执行。"""

    _instance: Optional["PluginManager"] = None
    _is_init_listener: bool = False
    _bot: Optional[Amia] = None
    _init: bool = False

    def __new__(cls, *args, **kwargs) -> "PluginManager":
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance

    def __init__(
        self,
        plugins_directory: str = "./plugins",
        cache_directory: str = "./cache/plugins",
        data_directory: str = "./data",
    ) -> None:
        """
        初始化PluginManager。

        Args:
            plugins_directory (str): 存储插件文件的目录。
            cache_directory (str): 用于缓存提取的插件文件的目录。
            config_directory (str): 存储插件配置文件的目录。
        """
        if self._init:
            return
        self.bot: Amia = Amia.get_instance()
        self.plugins_directory: Path = Path(plugins_directory)
        self.cache_directory: Path = Path(cache_directory)
        self.data_directory: Path = Path(data_directory)
        self.config_directory: Path = self.data_directory / "configs"
        self.plugin_config_directory: Path = self.config_directory / "plugins"
        self.loaded_plugins: Dict[str, Any] = {}  # 存储已加载的插件模块
        self.plugin_file_mapping: Dict[str, str] = {}  # 映射插件ID到文件名
        # 添加一个字典来缓存Plugin对象，避免重复读取ZIP文件
        self._plugins_cache: Dict[str, Plugin] = {}  # 缓存Plugin对象
        self.project_interface: ProjectInterface = ProjectInterface()

        # 初始化时清理缓存目录
        if self.cache_directory.exists():
            shutil.rmtree(self.cache_directory)

        self.plugins_directory.mkdir(exist_ok=True)
        self.cache_directory.mkdir(parents=True, exist_ok=True)
        self.plugin_config_directory.mkdir(parents=True, exist_ok=True)

        logging.info("PluginManager 初始化完成")
        if not self._is_init_listener:
            self.bot.listener(self._process_message)
            self._is_init_listener = True
        self._init = True

    async def _extract_plugin(self, plugin_zip_path: Path) -> str:
        """
        将插件ZIP文件提取到缓存目录并返回插件ID。

        Args:
            plugin_zip_path (Path): 插件ZIP文件的路径。

        Returns:
            str: 提取的插件的ID。
        """
        with zipfile.ZipFile(plugin_zip_path, "r") as zip_ref:
            # 查找info.json文件以获取ID
            plugin_info_string = zip_ref.read("info.json").decode("utf-8")
            plugin_info = json.loads(plugin_info_string)
            plugin_id = plugin_info["id"]

            extract_path = self.cache_directory / plugin_id

            # 增强的目录清理逻辑
            if extract_path.exists():
                retry_count = 0
                max_retries = 3
                while retry_count < max_retries:
                    try:
                        shutil.rmtree(extract_path)
                        break  # 成功删除，跳出循环
                    except Exception as e:
                        retry_count += 1
                        if retry_count >= max_retries:
                            logging.warning(f"清理插件缓存失败，将尝试直接覆盖: {e}")
                        else:
                            # 等待一段时间后重试
                            await asyncio.sleep(0.1)

            if extract_path.exists():
                shutil.rmtree(extract_path)

            # 确保目录存在，使用exist_ok=True避免创建已存在的目录时出错
            try:
                os.makedirs(extract_path, exist_ok=True)
            except Exception as e:
                logging.warning(f"创建插件缓存目录失败，将尝试继续: {e}")

            # 解压文件
            zip_ref.extractall(extract_path)
            return plugin_id

    async def _generate_plugin_config(
        self, plugin_id: str, plugin_info: Dict[str, Any]
    ) -> None:
        """
        为插件生成默认配置文件。

        Args:
            plugin_id (str): 插件的ID。
            plugin_info (Dict[str, Any]): 插件的信息。
        """
        config_file_path = self.plugin_config_directory / f"{plugin_id}.json"

        # 如果配置文件已存在，则不覆盖
        if config_file_path.exists():
            return

        # 从插件信息中提取triggers并生成默认配置
        default_config = {"triggers": {}}

        for trigger in plugin_info.get("triggers", []):
            trigger_id = trigger.get("id", trigger.get("name", "unknown"))
            default_config["triggers"][trigger_id] = {
                "enabled": True,
                "groups": [],
                "can_private": trigger.get("can_private", False),
            }
            if trigger.get("type") == "text_command":
                default_config["triggers"][trigger_id].update(
                    {
                        "must_prefix": trigger.get("params", {}).get(
                            "must_prefix", True
                        ),
                    }
                )

        # 写入配置文件
        with open(config_file_path, "w", encoding="utf-8") as f:
            json.dump(default_config, f, ensure_ascii=False)

        logging.info(f"为插件 '{plugin_id}' 生成了默认配置文件")

    async def _setup_plugin_file_mapping(self) -> None:
        """
        通过扫描插件目录设置插件文件映射。
        同时构建_plugins_cache字典，避免后续重复读取ZIP文件
        """
        # 清空现有映射和缓存
        self.plugin_file_mapping.clear()
        self._plugins_cache.clear()

        # 扫描启用的插件
        for plugin_file in self.plugins_directory.glob("*.plugin"):
            try:
                with zipfile.ZipFile(plugin_file, "r") as zip_ref:
                    plugin_info_string = zip_ref.read("info.json").decode("utf-8")
                    plugin_info = json.loads(plugin_info_string)
                    plugin_id = plugin_info["id"]
                    self.plugin_file_mapping[plugin_id] = plugin_file.name

                    # 创建Plugin对象并存储到缓存中
                    plugin_info.update(
                        {
                            "enabled": True,
                            "loaded": plugin_id in self.loaded_plugins,
                            "file_name": plugin_file.name,
                            "file_path": str(plugin_file),
                        }
                    )
                    self._plugins_cache[plugin_id] = Plugin(plugin_id, plugin_info)

                    # 生成默认配置文件
                    await self._generate_plugin_config(plugin_id, plugin_info)
            except Exception as e:
                logging.warning(
                    f"无法从 {plugin_file.name} 获取插件ID: {e}。使用文件名主干代替。"
                )
                plugin_id = plugin_file.stem
                self.plugin_file_mapping[plugin_id] = plugin_file.name

                # 创建基础Plugin对象并存储到缓存中
                self._plugins_cache[plugin_id] = Plugin(
                    plugin_id,
                    {
                        "name": plugin_id,
                        "description": "无法读取插件信息",
                        "enabled": True,
                        "loaded": plugin_id in self.loaded_plugins,
                        "file_name": plugin_file.name,
                        "file_path": str(plugin_file),
                    },
                )

        # 扫描禁用的插件
        for plugin_file in self.plugins_directory.glob("*.plugin.disabled"):
            try:
                with zipfile.ZipFile(plugin_file, "r") as zip_ref:
                    plugin_info_string = zip_ref.read("info.json").decode("utf-8")
                    plugin_info = json.loads(plugin_info_string)
                    plugin_id = plugin_info["id"]
                    self.plugin_file_mapping[plugin_id] = plugin_file.name.replace(
                        ".disabled", ""
                    )

                    # 创建Plugin对象并存储到缓存中
                    plugin_info.update(
                        {
                            "enabled": False,
                            "loaded": False,  # 禁用的插件不应被加载
                            "file_name": plugin_file.name,
                            "file_path": str(plugin_file),
                        }
                    )
                    self._plugins_cache[plugin_id] = Plugin(plugin_id, plugin_info)

                    # 生成默认配置文件
                    await self._generate_plugin_config(plugin_id, plugin_info)
            except Exception as e:
                logging.warning(
                    f"无法从 {plugin_file.name} 获取插件ID: {e}。使用文件名主干代替。"
                )
                plugin_id = plugin_file.stem.replace(".disabled", "")
                self.plugin_file_mapping[plugin_id] = plugin_file.name.replace(
                    ".disabled", ""
                )

                # 创建基础Plugin对象并存储到缓存中
                self._plugins_cache[plugin_id] = Plugin(
                    plugin_id,
                    {
                        "name": plugin_id,
                        "description": "无法读取插件信息",
                        "enabled": False,
                        "loaded": False,
                        "file_name": plugin_file.name,
                        "file_path": str(plugin_file),
                    },
                )

    async def _refresh_plugin_cache(self, plugin_id: str) -> None:
        """
        刷新特定插件在缓存中的信息

        Args:
            plugin_id (str): 要刷新的插件ID
        """
        if plugin_id not in self.plugin_file_mapping:
            # 如果插件文件映射中没有该插件，先执行_setup_plugin_file_mapping
            await self._setup_plugin_file_mapping()
            return

        # 确定插件文件路径
        plugin_file_name = self.plugin_file_mapping[plugin_id]
        is_enabled = True

        # 检查插件是否被禁用
        if not (self.plugins_directory / plugin_file_name).exists():
            # 尝试查找禁用的插件文件
            disabled_path = self.plugins_directory / f"{plugin_file_name}.disabled"
            if disabled_path.exists():
                plugin_file_name = disabled_path.name
                is_enabled = False

        plugin_file = self.plugins_directory / plugin_file_name

        try:
            # 尝试从插件文件中提取信息
            with zipfile.ZipFile(plugin_file, "r") as zip_ref:
                if "info.json" in zip_ref.namelist():
                    plugin_info_string = zip_ref.read("info.json").decode("utf-8")
                    plugin_info = json.loads(plugin_info_string)

                    # 更新插件信息
                    plugin_info.update(
                        {
                            "enabled": is_enabled,
                            "loaded": plugin_id in self.loaded_plugins,
                            "file_name": plugin_file.name,
                            "file_path": str(plugin_file),
                        }
                    )

                    # 更新缓存
                    self._plugins_cache[plugin_id] = Plugin(plugin_id, plugin_info)

                    # 生成默认配置文件
                    await self._generate_plugin_config(plugin_id, plugin_info)
        except Exception as e:
            logging.warning(f"刷新插件 '{plugin_id}' 缓存时出错: {e}")
            # 如果出错，创建一个基础的Plugin对象
            self._plugins_cache[plugin_id] = Plugin(
                plugin_id,
                {
                    "name": plugin_id,
                    "description": "无法读取插件信息",
                    "enabled": is_enabled,
                    "loaded": plugin_id in self.loaded_plugins,
                    "file_name": plugin_file.name,
                    "file_path": str(plugin_file),
                },
            )

    async def _load_plugin(self, plugin_zip_path: Path) -> bool:
        """
        从ZIP文件加载单个插件。

        Args:
            plugin_zip_path (Path): 插件ZIP文件的路径。

        Returns:
            bool: 如果插件成功加载则为True，否则为False。
        """
        max_retries = 3
        for attempt in range(max_retries):
            try:
                plugin_id = await self._extract_plugin(plugin_zip_path)

                # 动态加载模块
                if str(self.cache_directory) not in sys.path:
                    sys.path.append(str(self.cache_directory))

                if plugin_id in self.loaded_plugins:
                    # 重新加载现有模块
                    module = self.loaded_plugins[plugin_id]
                    importlib.reload(module)
                else:
                    # 首次加载模块
                    self.plugin_file_mapping[plugin_id] = plugin_zip_path.name
                    module = importlib.import_module(plugin_id)
                    self.loaded_plugins[plugin_id] = module

                # 将项目接口注入到插件模块
                setattr(module, "project_api", self.project_interface)

                # 刷新插件缓存
                await self._refresh_plugin_cache(plugin_id)

                # 确保缓存中的Plugin对象有正确的模块引用
                if plugin_id in self._plugins_cache:
                    self._plugins_cache[plugin_id].module = module

                logging.info(f"插件 '{plugin_id}' 加载成功。")
                return True
            except Exception as e:
                if attempt < max_retries - 1:
                    logging.warning(
                        f"加载插件 '{plugin_zip_path.name}' 失败(尝试 {attempt + 1}/{max_retries}): {e}\n错误堆栈:\n{traceback.format_exc()}"
                    )
                    await asyncio.sleep(0.1)  # Wait a short time before retrying
                else:
                    logging.error(
                        f"加载插件 '{plugin_zip_path.name}' 失败，已达到最大重试次数 {max_retries}: {e}\n错误堆栈:\n{traceback.format_exc()}"
                    )
            finally:
                # 确保加载后从sys.path中移除缓存路径
                if str(self.cache_directory) in sys.path:
                    sys.path.remove(str(self.cache_directory))

        return False

    async def load_all_plugins(self) -> None:
        """加载插件目录中的所有可用插件。"""
        await self._setup_plugin_file_mapping()
        shutil.rmtree(self.cache_directory, ignore_errors=True)
        self.plugins_directory.mkdir(parents=True, exist_ok=True)
        for plugin_file in self.plugins_directory.glob("*.plugin"):
            await self._load_plugin(plugin_file)

    async def reload_plugin(self, plugin_id: str) -> bool:
        """
        热重载特定插件。

        Args:
            plugin_id (str): 要重载的插件的ID。

        Returns:
            bool: 如果重载成功则为True，否则为False。
        """
        if plugin_id not in self.loaded_plugins:
            logging.warning(f"插件 '{plugin_id}' 当前未加载。")
            return False

        zip_path = self.plugins_directory / self.plugin_file_mapping[plugin_id]
        if not zip_path.exists():
            logging.error(f"找不到插件 '{plugin_id}' 的文件。无法重载。")
            return False

        result = await self._load_plugin(zip_path)
        # 重载后刷新缓存
        if result:
            await self._refresh_plugin_cache(plugin_id)
        return result

    async def reload_all_plugins(self) -> None:
        """重新加载所有已加载的插件。"""
        # 重新扫描并设置插件映射和缓存
        await self._setup_plugin_file_mapping()

        for plugin_id in list(self.loaded_plugins.keys()):
            await self.reload_plugin(plugin_id)

    async def unload_plugin(self, plugin_id: str) -> bool:
        """
        卸载特定插件并清理其缓存。

        Args:
            plugin_id (str): 要卸载的插件的ID。

        Returns:
            bool: 如果卸载成功则为True，否则为False。
        """
        if plugin_id in self.loaded_plugins:
            try:
                # 从字典中移除
                del self.loaded_plugins[plugin_id]

                # 从缓存中移除或更新插件状态
                if plugin_id in self._plugins_cache:
                    self._plugins_cache[plugin_id].loaded = False
                    self._plugins_cache[plugin_id].module = None

                # 清理缓存
                shutil.rmtree(self.cache_directory / plugin_id, ignore_errors=True)
                logging.info(f"插件 '{plugin_id}' 卸载成功。")
                return True
            except Exception as e:
                logging.error(f"卸载插件 '{plugin_id}' 失败: {e}")
                return False
        return False

    async def enable_plugin(self, plugin_id: str) -> bool:
        """
        通过重命名文件启用已禁用的插件。

        Args:
            plugin_id (str): 要启用的插件的ID。

        Returns:
            bool: 如果启用成功则为True，否则为False。
        """
        if plugin_id in self.plugin_file_mapping:
            disabled_path = (
                self.plugins_directory
                / f"{self.plugin_file_mapping[plugin_id]}.disabled"
            )
            if disabled_path.exists():
                disabled_path.rename(
                    self.plugins_directory / self.plugin_file_mapping[plugin_id]
                )
                await self._load_plugin(disabled_path)
                await self.reload_plugin(plugin_id)
                # 刷新缓存
                await self._refresh_plugin_cache(plugin_id)
                logging.info(f"插件 '{plugin_id}' 已启用。")
                return True
        return False

    async def disable_plugin(self, plugin_id: str) -> bool:
        """
        通过重命名文件禁用已启用的插件。

        Args:
            plugin_id (str): 要禁用的插件的ID。

        Returns:
            bool: 如果禁用成功则为True，否则为False。
        """
        if plugin_id in self.plugin_file_mapping:
            enabled_path = self.plugins_directory / self.plugin_file_mapping[plugin_id]
            if enabled_path.exists():
                enabled_path.rename(
                    self.plugins_directory
                    / f"{self.plugin_file_mapping[plugin_id]}.disabled"
                )
                await self.unload_plugin(plugin_id)
                # 刷新缓存
                await self._refresh_plugin_cache(plugin_id)
                logging.info(f"插件 '{plugin_id}' 已禁用。")
                return True
        return False

    async def call_plugin_function(
        self, plugin_id: str, function_name: str, **kwargs: Any
    ) -> Optional[Any]:
        """
        安全地调用已加载插件中的函数。

        Args:
            plugin_id (str): 插件的ID。
            function_name (str): 要调用的函数名称。
            **kwargs (Any): 传递给函数的关键字参数。

        Returns:
            Optional[Any]: 函数的返回值，如果发生错误则为None。
        """
        if plugin_id not in self.loaded_plugins:
            logging.error(f"错误: 插件 '{plugin_id}' 未加载。")
            return None

        module = self.loaded_plugins[plugin_id]
        if not hasattr(module, function_name):
            logging.error(
                f"错误: 在插件 '{plugin_id}' 中未找到函数 '{function_name}'。"
            )
            return None

        try:
            func = getattr(module, function_name)
            # 检查函数是否可等待并相应地调用
            if asyncio.iscoroutinefunction(func):
                return await func(**kwargs)
            else:
                return func(**kwargs)
        except Exception as e:
            logging.error(
                f"调用插件 '{plugin_id}' 中的函数 '{function_name}' 时出错: {e}\n"
                f"错误堆栈:\n{traceback.format_exc()}"
            )
            return None

    async def get_all_plugins_status(self) -> Dict[str, Any]:
        """
        获取所有插件的信息和状态。

        Returns:
            Dict[str, Dict[str, Any]]: 包含所有插件信息的字典，键为插件ID，值为插件的详细信息和状态。
        """
        # 如果缓存为空，先执行_setup_plugin_file_mapping
        if not self._plugins_cache:
            await self._setup_plugin_file_mapping()

        plugins_status = {}
        enabled_count = 0

        # 从缓存中获取插件状态
        for plugin_id, plugin in self._plugins_cache.items():
            plugins_status[plugin_id] = plugin.to_dict()
            if plugin.enabled:
                enabled_count += 1

        # 确保已加载但可能不在缓存中的插件也被包含
        for loaded_plugin_id in self.loaded_plugins:
            if loaded_plugin_id not in plugins_status:
                # 如果插件不在缓存中，创建基础信息
                plugins_status[loaded_plugin_id] = {
                    "id": loaded_plugin_id,
                    "enabled": False,
                    "loaded": True,
                    "file_name": self.plugin_file_mapping.get(loaded_plugin_id, "未知"),
                    "file_path": (
                        str(
                            self.plugins_directory
                            / self.plugin_file_mapping.get(loaded_plugin_id, "未知")
                        )
                        if loaded_plugin_id in self.plugin_file_mapping
                        else "未知"
                    ),
                }

        plugins_count = len(plugins_status)

        return {
            "plugins_count": plugins_count,
            "enabled_count": enabled_count,
            "plugins": plugins_status,
        }

    async def get_plugin(self, plugin_id: str) -> Optional[Plugin]:
        """
        获取指定插件ID的Plugin实例。

        Args:
            plugin_id (str): 要检索的插件的ID。

        Returns:
            Optional[Plugin]: Plugin实例，如果未找到则为None。
        """
        # 首先检查缓存中是否已有该插件
        if plugin_id in self._plugins_cache:
            # 确保缓存中的插件状态是最新的
            plugin = self._plugins_cache[plugin_id]
            plugin.loaded = plugin_id in self.loaded_plugins
            if plugin.loaded:
                plugin.module = self.loaded_plugins[plugin_id]
            return plugin

        # 如果没有，尝试通过_setup_plugin_file_mapping来刷新并获取
        await self._setup_plugin_file_mapping()

        # 再次检查缓存
        if plugin_id in self._plugins_cache:
            plugin = self._plugins_cache[plugin_id]
            plugin.loaded = plugin_id in self.loaded_plugins
            if plugin.loaded:
                plugin.module = self.loaded_plugins[plugin_id]
            return plugin

        return None

    async def get_all_plugins(self) -> List[Plugin]:
        """
        获取所有可用插件的Plugin实例列表。

        Returns:
            List[Plugin]: Plugin实例列表。
        """
        # 如果缓存为空，先执行_setup_plugin_file_mapping
        if not self._plugins_cache:
            await self._setup_plugin_file_mapping()

        # 更新缓存中所有插件的加载状态
        for plugin_id, plugin in self._plugins_cache.items():
            plugin.loaded = plugin_id in self.loaded_plugins
            if plugin.loaded:
                plugin.module = self.loaded_plugins[plugin_id]

        # 直接从缓存返回所有Plugin实例
        return list(self._plugins_cache.values())

    async def _process_message(self, message: Dict[str, Any]) -> None:
        """
        处理从Bot接收到的消息。
        此方法作为监听器注册到Amia实例。

        Args:
            message (Dict[str, Any]): 接收到的消息数据。
        """
        group_categories = Config(self.config_directory / "group_categories.json")
        if message.get("post_type") == "message":
            if "message_id" in message:
                msg = RecvMessage(message["message_id"], self.bot)
                await msg.get_info()
                if msg.user_id != 3381464350:
                    return
                for plugin in self._plugins_cache.values():
                    if plugin.enabled:
                        plugin_config = Config(
                            self.plugin_config_directory / f"{plugin.id}.json"
                        )
                        for trigger in plugin.triggers:
                            trigger_config = plugin_config.triggers[trigger["id"]]
                            # 检查群组是否在指定的分类中
                            is_in_valid_group = False
                            if msg.is_group and trigger_config.groups:
                                # 将分类ID转换为分类对象，再检查群组ID是否在其中
                                for category_id in trigger_config.groups:
                                    if (
                                        category_id in group_categories
                                        and msg.group_id
                                        in group_categories[category_id].get(
                                            "groups", []
                                        )
                                    ):
                                        is_in_valid_group = True
                                        logging.info(
                                            f"检查群组 {msg.group_id} 是否在分类 {category_id} 中: {is_in_valid_group}"
                                        )
                                        break

                            if trigger_config.enabled and (
                                is_in_valid_group
                                or (msg.is_private and trigger_config.can_private)
                            ):
                                if trigger["type"] == "text_pattern":
                                    if re.search(
                                        trigger["params"]["pattern"], msg.text.lower()
                                    ):
                                        with (self.data_directory / "usage.jsonl").open(
                                            "a", encoding="utf-8"
                                        ) as f:
                                            json.dump(
                                                {
                                                    "plugin_id": plugin.id,
                                                    "trigger_id": trigger["id"],
                                                    "message": msg.raw,
                                                },
                                                f,
                                                ensure_ascii=False,
                                            )
                                            f.write("\n")
                                        asyncio.create_task(
                                            plugin.call_function(
                                                trigger["func"], message=msg
                                            )
                                        )
                                elif trigger["type"] == "text_command":
                                    if (
                                        msg.text.lower().startswith(
                                            tuple(self.bot.config.prefixes)
                                        )
                                        and msg.text
                                        and msg.text.lower()[1:].startswith(
                                            trigger["params"]["command"]
                                        )
                                    ) or (
                                        not trigger_config.get("must_prefix", True)
                                        and msg.text.lower().startswith(
                                            trigger["params"]["command"]
                                        )
                                    ):
                                        with (self.data_directory / "usage.jsonl").open(
                                            "a", encoding="utf-8"
                                        ) as f:
                                            json.dump(
                                                {
                                                    "plugin_id": plugin.id,
                                                    "trigger_id": trigger["id"],
                                                    "message": msg.raw,
                                                },
                                                f,
                                                ensure_ascii=False,
                                            )
                                            f.write("\n")
                                        asyncio.create_task(
                                            plugin.call_function(
                                                trigger["func"], message=msg
                                            )
                                        )
        # if message.get("user_id") == 337374551:
        #     breakpoint()
        for plugin in self._plugins_cache.values():
            if plugin.enabled:
                plugin_config = Config(
                    self.plugin_config_directory / f"{plugin.id}.json"
                )
                for trigger in plugin.triggers:
                    trigger_config = plugin_config.triggers[trigger["id"]]
                    # 检查群组是否在指定的分类中
                    is_in_valid_group = False
                    if message.get("group_id") and trigger_config.groups:
                        # 将分类ID转换为分类对象，再检查群组ID是否在其中
                        for category_id in trigger_config.groups:
                            if category_id in group_categories and message.get(
                                "group_id"
                            ) in group_categories[category_id].get("groups", []):
                                is_in_valid_group = True
                                logging.info(
                                    f"检查群组 {message.get('group_id')} 是否在分类 {category_id} 中: {is_in_valid_group}"
                                )
                                break

                    if trigger_config.enabled and (
                        is_in_valid_group
                        or (
                            message.get("group_id") is None
                            and trigger_config.can_private
                        )
                    ):
                        if trigger["type"] == "match_message":
                            # 检查触发器是否匹配
                            is_trigger_matched = False

                            # 处理字段精确匹配
                            if "matches" in trigger["params"]:
                                is_trigger_matched = recursive_match(
                                    message,
                                    trigger["params"]["matches"],
                                    trigger["params"].get("array_match_type", "all"),
                                )

                            # 如果触发器匹配，则执行相应的函数
                            if is_trigger_matched:
                                with (self.data_directory / "usage.jsonl").open(
                                    "a", encoding="utf-8"
                                ) as f:
                                    json.dump(
                                        {
                                            "plugin_id": plugin.id,
                                            "trigger_id": trigger["id"],
                                            "message": message,
                                        },
                                        f,
                                        ensure_ascii=False,
                                    )
                                    f.write("\n")
                                asyncio.create_task(
                                    plugin.call_function(
                                        trigger["func"], message=message
                                    )
                                )
        # logging.info(f"处理消息结束: {message}")
