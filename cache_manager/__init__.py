from pathlib import Path
from typing import Optional
from time import time
import hashlib
from uuid import uuid4
import os


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
        if CacheManager._instance is None:
            CacheManager._instance = self
        self.cache_dir = cache_dir
        self.cache_dir.mkdir(parents=True, exist_ok=True)

    def get_cache(self, ext: str = "file") -> Path:
        filename = f"{time()}-{uuid4()}.{ext}"
        return self.cache_dir / filename

    def _is_expired(self, path: Path, ttl: Optional[int]) -> bool:
        """判断文件是否过期"""
        if ttl is None:
            return False
        if not path.exists():
            return True  # 不存在也视为“过期”
        modified_time = os.path.getmtime(path)
        return (time() - modified_time) > ttl

    def get_cache_by_id(self, id: str, ext: str = "file", ttl: Optional[int] = None) -> Path:
        """按 ID 获取缓存路径"""
        key = hashlib.sha256(id.encode("utf-8")).hexdigest()
        path = self.cache_dir / f"{key}.{ext}"
        if self._is_expired(path, ttl):
            path.unlink(missing_ok=True)
        return path

    def get_cache_by_filename(self, filename: str, ttl: Optional[int] = None) -> Path:
        """按文件名获取缓存路径"""
        path = self.cache_dir / filename
        if self._is_expired(path, ttl):
            path.unlink(missing_ok=True)
        return path

    def get_child_cache(self, id: str) -> "CacheManager":
        return CacheManager(self.cache_dir / id)
