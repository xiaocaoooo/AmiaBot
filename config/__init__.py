from datetime import datetime, timedelta
import json
from pathlib import Path
from datetime import datetime, timedelta
from typing import Any, Dict, List, Union, Optional


class ConfigObject:
    """配置对象基类，提供字典式访问和递归转换功能"""
    def __init__(self, data: Dict[str, Any]):
        """初始化ConfigObject实例
        
        Args:
            data: 配置数据字典
        """
        self._data = data
        for key, value in data.items():
            if isinstance(value, dict):
                self._data[key] = ConfigObject(value)
            elif isinstance(value, list):
                self._data[key] = [ConfigObject(item) if isinstance(item, dict) else item for item in value]
            else:
                self._data[key] = value

        
    def __getattr__(self, name: str) -> Any:
        """通过属性访问配置值
        
        Args:
            name: 属性名
        
        Returns:
            属性值，如果不存在则返回None
        """
        return self._data.get(name)
    
    def __getitem__(self, name: str) -> Any:
        """通过索引访问配置值
        
        Args:
            name: 索引名
        
        Returns:
            索引对应的值
        """
        return self.__getattr__(name)
    
    def __setattr__(self, name: str, value: Any) -> None:
        """设置配置属性值
        
        Args:
            name: 属性名
            value: 属性值
        """
        # 对于内部_data属性，直接调用父类方法设置
        if name == '_data':
            super().__setattr__(name, value)
        else:
            # 确保_data已经初始化
            if hasattr(self, '_data'):
                self._data[name] = value
            else:
                # 如果_data还未初始化，先调用父类方法设置
                super().__setattr__(name, value)
    
    def __setitem__(self, name: str, value: Any) -> None:
        """通过索引设置配置值
        
        Args:
            name: 索引名
            value: 要设置的值
        """
        self.__setattr__(name, value)
        
    def toDict(self) -> Dict[str, Any]:
        """将配置对象转换为普通字典
        
        Returns:
            配置数据字典
        """
        return self._data
    
    def __repr__(self) -> str:
        """返回配置对象的字符串表示
        
        Returns:
            配置对象的字符串表示
        """
        return self._data.__repr__()

class Config(ConfigObject):
    """配置管理类，扩展ConfigObject，提供文件读写和缓存功能"""
    _last_update_time: datetime  # 最后更新时间
    
    def __init__(self, filename: Path) -> None:
        """初始化Config实例
        
        Args:
            filename: 配置文件路径
        """
        super().__init__(json.loads(filename.read_text()))
        self._filename = filename
        self._last_update_time = datetime.now()
        
    def __getattr__(self, name: str) -> Any:
        """获取配置属性，支持缓存机制
        
        Args:
            name: 属性名
        
        Returns:
            属性值，如果不存在则返回None
        """
        # 对于info_cache_time属性，提供默认值0
        if name == 'info_cache_time':
            return self._data.get(name, 0)
        
        # 检查是否需要刷新缓存
        if name not in ['_last_update_time', '_filename'] and hasattr(self, '_last_update_time') and hasattr(self, 'info_cache_time'):
            if datetime.now() - self._last_update_time > timedelta(milliseconds=self.info_cache_time):
                self.fresh()
        # 调用父类方法获取属性
        return super().__getattr__(name)
    
    def __setattr__(self, name: str, value: Any) -> None:
        """设置配置属性，并自动保存到文件
        
        Args:
            name: 属性名
            value: 属性值
        """
        # 对于特殊属性，直接调用父类方法设置，避免递归
        if name in ['_last_update_time', '_filename', '_data']:
            super().__setattr__(name, value)
        else:
            # 其他属性先设置，然后更新时间和写入文件
            super().__setattr__(name, value)
            # 确保_last_update_time和_filename已经初始化
            if hasattr(self, '_last_update_time') and hasattr(self, '_filename'):
                self._last_update_time = datetime.now()
                self._filename.write_text(json.dumps(self.toDict()))

    def fresh(self) -> None:
        """刷新配置缓存，重新从文件加载配置
        
        重新加载配置文件，而不是重新初始化对象
        """
        if hasattr(self, '_filename'):
            new_data = json.loads(self._filename.read_text())
            # 递归转换嵌套字典为ConfigObject
            for key, value in new_data.items():
                if isinstance(value, dict):
                    new_data[key] = ConfigObject(value)
                elif isinstance(value, list):
                    new_data[key] = [ConfigObject(item) if isinstance(item, dict) else item for item in value]
            self._data = new_data
            self._last_update_time = datetime.now()
