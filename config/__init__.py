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
        return self._data.get(name)
    
    def __setattr__(self, name: str, value):
        if name == '_data':
            super().__setattr__(name, value)
        else:
            self._data[name] = value
    
    def __setitem__(self, name: str, value):
        self._data[name] = value
        
    def toDict(self):
        return self._data
    
    def __repr__(self):
        return self._data.__repr__()

class Config(ConfigObject):
    def __init__(self, filename: Path):
        super().__init__(json.loads(filename.read_text()))
        self._filename = filename

    def fresh(self):
        self.__init__(self._filename)
