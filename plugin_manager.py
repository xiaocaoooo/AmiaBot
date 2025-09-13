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
    bot:Amia|None = None

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

    def enable_plugin(self, plugin_id: str):
        """
        Enables a disabled plugin by renaming its file.

        Args:
            plugin_id (str): The ID of the plugin to enable.
        """
        disabled_path = self.plugins_directory / f"{plugin_id}.plugin.disabled"
        if disabled_path.exists():
            disabled_path.rename(self.plugins_directory / f"{plugin_id}.plugin")
            logging.info(f"Plugin '{plugin_id}' enabled.")

    def disable_plugin(self, plugin_id: str):
        """
        Disables an enabled plugin by renaming its file.

        Args:
            plugin_id (str): The ID of the plugin to disable.
        """
        enabled_path = self.plugins_directory / f"{plugin_id}.plugin"
        if enabled_path.exists():
            enabled_path.rename(self.plugins_directory / f"{plugin_id}.plugin.disabled")
            logging.info(f"Plugin '{plugin_id}' disabled.")

    async def call_plugin_function(
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
