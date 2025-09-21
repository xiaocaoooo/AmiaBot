import os
import shutil
import importlib
import sys
import zipfile
import json
import asyncio
import logging
from pathlib import Path
from typing import Dict, Any, Optional

from amia import Amia


# 定义项目接口，供插件调用
class ProjectInterface:
    """Interface for plugins to interact with the main project."""

    _instance = None
    bot: Amia | None = None

    def __new__(cls, *args, **kwargs):
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance

    def __init__(self):
        pass

    async def send_data_to_project(self, data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Example: A plugin calls this to send data to the project.

        Args:
            data (Dict[str, Any]): The data to be sent.

        Returns:
            Dict[str, Any]: A response dictionary.
        """
        logging.info(f"Project received data from plugin: {json.dumps(data)}")
        # Here, you would implement the specific business logic, like writing to a database or sending a network request.
        return {"status": "success", "message": "Data received."}


class PluginManager:
    """Manages the lifecycle of plugins, including loading, unloading, and execution."""

    _instance = None

    def __new__(cls, *args, **kwargs):
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance

    def __init__(
        self,
        plugins_directory: str = "./plugins",
        cache_directory: str = "./cache/plugins",
    ):
        """
        Initializes the PluginManager.

        Args:
            plugins_directory (str): The directory where plugin files are stored.
            cache_directory (str): The directory for caching extracted plugin files.
        """
        self.plugins_directory = Path(plugins_directory)
        self.cache_directory = Path(cache_directory)
        self.loaded_plugins: Dict[str, Any] = {}
        self.plugin_file_mapping: Dict[str, str] = {}
        self.project_interface = ProjectInterface()

        # Clean up the cache directory on initialization
        if self.cache_directory.exists():
            shutil.rmtree(self.cache_directory)

        self.plugins_directory.mkdir(exist_ok=True)
        self.cache_directory.mkdir(parents=True, exist_ok=True)

    async def _extract_plugin(self, plugin_zip_path: Path) -> str:
        """
        Extracts a plugin ZIP file to the cache directory and returns the plugin ID.

        Args:
            plugin_zip_path (Path): The path to the plugin's ZIP file.

        Returns:
            str: The ID of the extracted plugin.
        """
        with zipfile.ZipFile(plugin_zip_path, "r") as zip_ref:
            # Find the info.json file to get the ID
            plugin_info_string = zip_ref.read("info.json").decode("utf-8")
            plugin_info = json.loads(plugin_info_string)
            plugin_id = plugin_info["id"]

            extract_path = self.cache_directory / plugin_id
            if extract_path.exists():
                shutil.rmtree(extract_path)  # Clean up old cache
            os.makedirs(extract_path, exist_ok=True)
            zip_ref.extractall(extract_path)
            return plugin_id
        
    async def _setup_plugin_file_mapping(self):
        """
        Sets up the plugin file mapping by scanning the plugins directory.
        """
        for plugin_file in self.plugins_directory.glob("*.plugin"):
            try:
                with zipfile.ZipFile(plugin_file, "r") as zip_ref:
                    plugin_info_string = zip_ref.read("info.json").decode("utf-8")
                    plugin_info = json.loads(plugin_info_string)
                    plugin_id = plugin_info["id"]
                    self.plugin_file_mapping[plugin_id] = plugin_file.name
            except Exception as e:
                logging.warning(f"Failed to get plugin ID from {plugin_file.name}: {e}. Using filename stem instead.")
                plugin_id = plugin_file.stem
                self.plugin_file_mapping[plugin_id] = plugin_file.name

        for plugin_file in self.plugins_directory.glob("*.plugin.disabled"):
            try:
                with zipfile.ZipFile(plugin_file, "r") as zip_ref:
                    plugin_info_string = zip_ref.read("info.json").decode("utf-8")
                    plugin_info = json.loads(plugin_info_string)
                    plugin_id = plugin_info["id"]
                    self.plugin_file_mapping[plugin_id] = plugin_file.name.replace(".disabled", "")
            except Exception as e:
                logging.warning(f"Failed to get plugin ID from {plugin_file.name}: {e}. Using filename stem instead.")
                plugin_id = plugin_file.stem.replace(".disabled", "")
                self.plugin_file_mapping[plugin_id] = plugin_file.name.replace(".disabled", "")

    async def _load_plugin(self, plugin_zip_path: Path) -> bool:
        """
        Loads a single plugin from a ZIP file.

        Args:
            plugin_zip_path (Path): The path to the plugin's ZIP file.

        Returns:
            bool: True if the plugin was loaded successfully, False otherwise.
        """
        try:
            plugin_id = await self._extract_plugin(plugin_zip_path)

            # Dynamically load the module
            if str(self.cache_directory) not in sys.path:
                sys.path.append(str(self.cache_directory))

            if plugin_id in self.loaded_plugins:
                # Reload the existing module
                module = self.loaded_plugins[plugin_id]
                importlib.reload(module)
            else:
                # First time loading the module
                self.plugin_file_mapping[plugin_id] = plugin_zip_path.name
                module = importlib.import_module(plugin_id)
                self.loaded_plugins[plugin_id] = module

            # Inject the project interface into the plugin module
            setattr(module, "project_api", self.project_interface)

            logging.info(f"Plugin '{plugin_id}' loaded successfully.")
            return True
        except Exception as e:
            logging.error(f"Failed to load plugin '{plugin_zip_path.name}': {e}")
            return False
        finally:
            # Ensure the cache path is removed from sys.path after loading
            if str(self.cache_directory) in sys.path:
                sys.path.remove(str(self.cache_directory))

    async def load_all_plugins(self):
        """Loads all available plugins from the plugins directory."""
        await self._setup_plugin_file_mapping()
        self.plugins_directory.mkdir(parents=True, exist_ok=True)
        for plugin_file in self.plugins_directory.glob("*.plugin"):
            await self._load_plugin(plugin_file)

    async def reload_plugin(self, plugin_id: str) -> bool:
        """
        Hot reloads a specific plugin.

        Args:
            plugin_id (str): The ID of the plugin to reload.

        Returns:
            bool: True if the reload was successful, False otherwise.
        """
        if plugin_id not in self.loaded_plugins:
            logging.warning(f"Plugin '{plugin_id}' is not currently loaded.")
            return False

        zip_path = self.plugins_directory / self.plugin_file_mapping[plugin_id]
        if not zip_path.exists():
            logging.error(f"Plugin file for '{plugin_id}' not found. Cannot reload.")
            return False

        return await self._load_plugin(zip_path)
    
    async def reload_all_plugins(self):
        """Reloads all loaded plugins."""
        for plugin_id in list(self.loaded_plugins.keys()):
            await self.reload_plugin(plugin_id)

    async def unload_plugin(self, plugin_id: str) -> bool:
        """
        Unloads a specific plugin and cleans up its cache.

        Args:
            plugin_id (str): The ID of the plugin to unload.

        Returns:
            bool: True if the unload was successful, False otherwise.
        """
        if plugin_id in self.loaded_plugins:
            try:
                # Remove from the dictionary
                del self.loaded_plugins[plugin_id]
                del self.plugin_file_mapping[plugin_id]
                # Clean up the cache
                shutil.rmtree(self.cache_directory / plugin_id, ignore_errors=True)
                logging.info(f"Plugin '{plugin_id}' unloaded successfully.")
                return True
            except Exception as e:
                logging.error(f"Failed to unload plugin '{plugin_id}': {e}")
                return False
        return False

    async def enable_plugin(self, plugin_id: str):
        """
        Enables a disabled plugin by renaming its file.

        Args:
            plugin_id (str): The ID of the plugin to enable.
        """
        print(self.plugin_file_mapping)
        if plugin_id in self.plugin_file_mapping:
            disabled_path = self.plugins_directory / f"{self.plugin_file_mapping[plugin_id]}.disabled"
            if disabled_path.exists():
                disabled_path.rename(self.plugins_directory / self.plugin_file_mapping[plugin_id])
                await self._load_plugin(disabled_path)
                await self.reload_plugin(plugin_id)
                logging.info(f"Plugin '{plugin_id}' enabled.")
                return True
        return False

    async def disable_plugin(self, plugin_id: str):
        """
        Disables an enabled plugin by renaming its file.

        Args:
            plugin_id (str): The ID of the plugin to disable.
        """
        if plugin_id in self.plugin_file_mapping:
            enabled_path = self.plugins_directory / self.plugin_file_mapping[plugin_id]
            if enabled_path.exists():
                enabled_path.rename(self.plugins_directory / f"{self.plugin_file_mapping[plugin_id]}.disabled")
                await self.unload_plugin(plugin_id)
                logging.info(f"Plugin '{plugin_id}' disabled.")
                return True
        return False

    async  def call_plugin_function(
        self, plugin_id: str, function_name: str, **kwargs: Any
    ) -> Optional[Any]:
        """
        Safely calls a function within a loaded plugin.

        Args:
            plugin_id (str): The ID of the plugin.
            function_name (str): The name of the function to call.
            **kwargs (Any): Keyword arguments to pass to the function.

        Returns:
            Optional[Any]: The return value of the function, or None if an error occurred.
        """
        if plugin_id not in self.loaded_plugins:
            logging.error(f"Error: Plugin '{plugin_id}' is not loaded.")
            return None

        module = self.loaded_plugins[plugin_id]
        if not hasattr(module, function_name):
            logging.error(
                f"Error: Function '{function_name}' not found in plugin '{plugin_id}'."
            )
            return None

        try:
            func = getattr(module, function_name)
            # Check if the function is awaitable and call it accordingly
            if asyncio.iscoroutinefunction(func):
                return await func(**kwargs)
            else:
                return func(**kwargs)
        except Exception as e:
            logging.error(
                f"Error calling function '{function_name}' in plugin '{plugin_id}': {e}"
            )
            return None

    async def get_all_plugins_status(self) -> Dict[str, Any]:
        """
        获取所有插件的信息和状态。

        Returns:
            Dict[str, Dict[str, Any]]: 包含所有插件信息的字典，键为插件ID，值为插件的详细信息和状态。
        """
        plugins_status = {}
        enabled_count = len(list(self.plugins_directory.glob("*.plugin")))
        plugins_count = enabled_count + len(
            list(self.plugins_directory.glob("*.plugin.disabled"))
        )

        # 扫描插件目录中的所有插件文件
        enabled_plugins = list(self.plugins_directory.glob("*.plugin"))
        disabled_plugins = list(self.plugins_directory.glob("*.plugin.disabled"))

        # 处理所有插件
        for plugin_file in enabled_plugins + disabled_plugins:
            is_enabled = plugin_file.suffix == ".plugin"
            plugin_id = None
            plugin_info = {}

            try:
                # 尝试从插件文件中提取信息
                with zipfile.ZipFile(plugin_file, "r") as zip_ref:
                    if "info.json" in zip_ref.namelist():
                        plugin_info_string = zip_ref.read("info.json").decode("utf-8")
                        plugin_info = json.loads(plugin_info_string)
                        plugin_id = plugin_info["id"]
            except Exception as e:
                # 如果无法读取info.json，尝试从文件名推断插件ID
                logging.warning(f"无法读取插件文件 '{plugin_file.name}' 的信息: {e}")
                plugin_id = plugin_file.stem.replace(".plugin", "").replace(
                    ".disabled", ""
                )

            if plugin_id:
                # 收集插件状态
                plugins_status[plugin_id] = {
                    **plugin_info,
                    "enabled": is_enabled,
                    "loaded": plugin_id in self.loaded_plugins,
                    "file_name": plugin_file.name,
                    "file_path": str(plugin_file),
                }

        # 确保已加载但可能不在插件目录中的插件也被包含
        for loaded_plugin_id in self.loaded_plugins:
            if loaded_plugin_id not in plugins_status:
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

        return {
            "plugins_count": plugins_count,
            "enabled_count": enabled_count,
            "plugins": plugins_status,
        }
