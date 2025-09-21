from datetime import datetime, timedelta
import json
from pathlib import Path


class ConfigObject:
    def __init__(self, data: dict):
        self._data = data
        for key, value in data.items():
            if isinstance(value, dict):
                self._data[key] = ConfigObject(value)
            elif isinstance(value, list):
                self._data[key] = [ConfigObject(item) if isinstance(item, dict) else item for item in value]
            else:
                self._data[key] = value

        
    def __getattr__(self, name: str):
        return self._data.get(name)
    
    def __getitem__(self, name: str):
        return self.__getattr__(name)
    
    def __setattr__(self, name: str, value):
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
    
    def __setitem__(self, name: str, value):
        self.__setattr__(name, value)
        
    def toDict(self):
        return self._data
    
    def __repr__(self):
        return self._data.__repr__()

class Config(ConfigObject):
    _last_update_time: datetime
    
    def __init__(self, filename: Path):
        super().__init__(json.loads(filename.read_text()))
        self._filename = filename
        self._last_update_time = datetime.now()
        
    def __getattr__(self, name: str):
        # 对于info_cache_time属性，提供默认值0
        if name == 'info_cache_time':
            return self._data.get(name, 0)
        
        # 检查是否需要刷新缓存
        if name not in ['_last_update_time', '_filename'] and hasattr(self, '_last_update_time') and hasattr(self, 'info_cache_time'):
            if datetime.now() - self._last_update_time > timedelta(milliseconds=self.info_cache_time):
                self.fresh()
        # 调用父类方法获取属性
        return super().__getattr__(name)
    
    def __setattr__(self, name: str, value):
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

    def fresh(self):
        # 重新加载配置文件，而不是重新初始化对象
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
