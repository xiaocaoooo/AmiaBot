from datetime import datetime, timedelta
import json
from pathlib import Path
from datetime import datetime, timedelta
from typing import Any, Dict, List, Union, Optional


class ConfigObject:
    """配置对象基类，提供字典式访问和递归转换功能（只读）"""
    _data: Dict[str, Any] | List[Any]
    
    def __init__(self, data: Dict[str, Any] | List[Any]):
        """初始化ConfigObject实例
        
        Args:
            data: 配置数据字典或列表
        """
        self._data = data
        if isinstance(data, dict):
            for key, value in list(data.items()):
                if isinstance(value, dict):
                    data[key] = ConfigObject(value)
                elif isinstance(value, list):
                    data[key] = [ConfigObject(item) if isinstance(item, (dict, list)) else item for item in value]
        elif isinstance(data, list):
            for i, item in enumerate(data):
                if isinstance(item, (dict, list)):
                    data[i] = ConfigObject(item)

        
    def __getattr__(self, name: str) -> Any:
        """通过属性访问配置值
        
        Args:
            name: 属性名
        
        Returns:
            属性值，如果不存在则返回None
        """
        if isinstance(self._data, dict):
            return self._data.get(name)
        else:
            # 如果不是字典，尝试将name转为整数作为索引访问
            try:
                index = int(name)
                if 0 <= index < len(self._data):
                    return self._data[index]
            except (ValueError, IndexError):
                pass
            return None
    
    def __getitem__(self, key: Union[str, int]) -> Any:
        """通过索引访问配置值
        
        Args:
            key: 索引名或索引位置
        
        Returns:
            索引对应的值
        """
        if isinstance(self._data, dict) and isinstance(key, str):
            return self._data.get(key)
        elif isinstance(self._data, list) and isinstance(key, int):
            try:
                return self._data[key]
            except IndexError:
                return None
        return None
    
    def get(self, key: str, default: Any = None) -> Any:
        """获取配置值
        
        Args:
            key: 配置键名
            default: 如果键不存在时返回的默认值，默认为None
            
        Returns:
            配置值，如果不存在则返回默认值
        """
        if isinstance(self._data, dict):
            return self._data.get(key, default)
        return default
    
    def __setattr__(self, name: str, value: Any) -> None:
        """设置配置属性值（只读模式，禁止修改）
        
        Args:
            name: 属性名
            value: 属性值
            
        Raises:
            AttributeError: 当尝试修改只读配置对象时抛出
        """
        # 允许设置内部特殊属性
        if name in ['_data', '_last_update_time', '_filename']:
            # 对于_data属性，只允许在初始化阶段设置一次
            if name == '_data':
                try:
                    # 尝试直接获取_data，如果不存在会抛出AttributeError
                    object.__getattribute__(self, '_data')
                    # 如果没有抛出异常，说明_data已经存在，不允许修改
                    raise AttributeError("Cannot modify read-only ConfigObject")
                except AttributeError:
                    # 如果抛出异常，说明_data还不存在，允许设置
                    super().__setattr__(name, value)
            else:
                # 对于其他特殊属性，允许设置
                super().__setattr__(name, value)
        else:
            # 其他情况都不允许修改，抛出异常
            raise AttributeError("Cannot modify read-only ConfigObject")
    
    def __setitem__(self, key: Union[str, int], value: Any) -> None:
        """通过索引设置配置值（只读模式，禁止修改）
        
        Args:
            key: 索引名或索引位置
            value: 要设置的值
            
        Raises:
            TypeError: 当尝试修改只读配置对象时抛出
        """
        raise TypeError("Cannot modify read-only ConfigObject")
        
    def toDict(self) -> Union[Dict[str, Any], List[Any]]:
        """将配置对象转换为普通字典或列表
        
        Returns:
            配置数据字典或列表
        """
        if isinstance(self._data, dict):
            dict_result: Dict[str, Any] = {}
            for key, value in self._data.items():
                if isinstance(value, ConfigObject):
                    dict_result[key] = value.toDict()
                else:
                    dict_result[key] = value
            return dict_result
        elif isinstance(self._data, list):
            list_result: List[Any] = []
            for item in self._data:
                if isinstance(item, ConfigObject):
                    list_result.append(item.toDict())
                else:
                    list_result.append(item)
            return list_result
        return self._data
    
    def __repr__(self) -> str:
        """返回配置对象的字符串表示
        
        Returns:
            配置对象的字符串表示
        """
        return self._data.__repr__()
    
    def __contains__(self, item: str) -> bool:
        """检查配置对象是否包含指定的键
        
        Args:
            item: 键名
        
        Returns:
            如果包含则返回True，否则返回False
        """
        if isinstance(self._data, dict):
            return item in self._data
        elif isinstance(self._data, list):
            # 如果是列表，尝试将item转为整数作为索引访问
            try:
                index = int(item)
                if 0 <= index < len(self._data):
                    return True
            except (ValueError, IndexError):
                pass
        return False

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
        # 直接检查特殊属性，避免递归调用
        if name == '_last_update_time' or name == '_filename' or name == '_data':
            return super().__getattribute__(name)
        
        # 对于info_cache_time属性，提供默认值0
        if name == 'info_cache_time':
            if isinstance(self._data, dict):
                return self._data.get(name, 0)
            return 0
        
        # 检查是否需要刷新缓存
        try:
            last_update_time = super().__getattribute__('_last_update_time')
            # 从_data直接获取info_cache_time
            info_cache_time = 0
            if isinstance(self._data, dict) and 'info_cache_time' in self._data:
                info_cache_time = self._data['info_cache_time']
            
            if datetime.now() - last_update_time > timedelta(milliseconds=info_cache_time):
                self.fresh()
        except AttributeError:
            # 如果_last_update_time还未初始化，跳过刷新
            pass
        
        # 直接从_data获取属性，避免调用父类的__getattr__导致递归
        if isinstance(self._data, dict):
            return self._data.get(name)
        elif isinstance(self._data, list):
            # 如果是列表，尝试将name转为整数作为索引访问
            try:
                index = int(name)
                if 0 <= index < len(self._data):
                    return self._data[index]
            except (ValueError, IndexError):
                pass
        return None
    
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
