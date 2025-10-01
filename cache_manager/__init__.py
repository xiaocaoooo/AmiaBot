from pathlib import Path
from typing import Optional, Dict
from time import time


class CacheManager:
    _instance: Optional["CacheManager"] = None
    
    @classmethod
    def has_instance(cls) -> bool:
        return cls._instance is not None
    
    @classmethod
    def get_instance(cls, cache_dir: Optional[Path] = None) -> "CacheManager":
        if cls._instance is None:
            if cache_dir is None:
                raise ValueError("cache_dir is None")
            cls._instance = CacheManager(cache_dir)
        return cls._instance
    
    def __init__(self, cache_dir: Path):
        self.cache_dir = cache_dir
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        CacheManager._instance = self

    def get_cache(self, ext: str = "file") -> Path:
        return self.cache_dir / f"{time()}.{ext}"

    def get_cache_by_id(self, id: str, ext: str = "file") -> Path:
        return self.cache_dir / f"{hash(id)}.{ext}"
    def get_cache_by_filename(self, filename: str) -> Path:
        return self.cache_dir / filename

    def get_child_cache(self, id: str) -> "CacheManagerChild":
        return CacheManagerChild(self.cache_dir / id)


class CacheManagerChild(CacheManager):
    _instances: Dict[str, "CacheManagerChild"] = {}
    
    def __init__(self, cache_dir: Path):
        self.cache_dir = cache_dir
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        CacheManagerChild._instances[str(cache_dir)] = self
