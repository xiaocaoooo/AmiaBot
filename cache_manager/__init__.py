from pathlib import Path
from typing import Optional
from time import time


class CacheManager:
    _instance: Optional["CacheManager"] = None
    
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
