import logging
from pathlib import Path
import re
import threading
import traceback
from typing import Dict, Iterator, Optional
from time import sleep, time
import hashlib
from uuid import uuid4
import os
from config import Config


cache_info_file = Path("cache_info.json")
cache_info = Config(cache_info_file)


class CacheManager:
    _instance: Optional["CacheManager"] = None
    _file_stats:Dict[str, os.stat_result]={}

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
            threading.Thread(target=self.schedule_clean_cache, daemon=True).start()
        self.cache_dir = cache_dir
        self.cache_dir.mkdir(parents=True, exist_ok=True)

    def get_cache_size(self) -> int:
        """获取缓存目录大小（单位：字节）"""
        total_size = 0
        compiled_patterns = [re.compile(i) for i in cache_info.ignore]
        for file in iterdir(self.cache_dir):
            if file.is_file():
                relative_path = str(file.relative_to(self.cache_dir).as_posix())
                for pattern in compiled_patterns:
                    if pattern.match(relative_path):
                        # print(f"ignore: {relative_path}")
                        break
                else:
                    # print(f"match: {relative_path}")
                    self._file_stats[relative_path] = file.stat()
                    total_size += self._file_stats[relative_path].st_size
        return total_size
    
    def schedule_clean_cache(self):
        """计划清理过期缓存"""
        while 1:
            try:
                logging.info("clean_cache start")
                deleted = self.clean_cache()
                logging.info(f"clean_cache end, deleted: {deleted}")
            except Exception as e:
                logging.error(f"clean_cache error: {e}\n{traceback.format_exc()}")
            finally:
                sleep(cache_info.interval)
    
    def clean_cache(self)->int:
        """清理过期缓存"""
        deleted=0
        if cache_info.clean_mode=="size":
            total_size = self.get_cache_size()
            sorted_files = sorted(
                self._file_stats.items(), key=lambda x: x[1].st_mtime
            )
            while total_size>cache_info.max_cache_size:
                relative_path, stat = sorted_files.pop(0)
                total_size -= stat.st_size
                deleted+=1
                logging.info(f"delete: {relative_path}")
                (self.cache_dir / relative_path).unlink(missing_ok=True)
                self._file_stats.pop(relative_path, None)
        return deleted

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

    def get_cache_by_id(
        self, id: str, ext: str = "file", ttl: Optional[int] = None
    ) -> Path:
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

def iterdir(path: Path) -> Iterator[Path]:
    """递归遍历目录，返回所有文件路径"""
    for item in path.iterdir():
        if item.is_dir():
            yield from iterdir(item)
        else:
            yield item
