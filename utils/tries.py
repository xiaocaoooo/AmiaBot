import asyncio
import logging
import traceback


def tries(tries: int = 5):
    def decorator(func):
        async def wrapper(*args, **kwargs):
            for time in range(tries):
                try:
                    return await func(*args, **kwargs)
                except Exception as e:
                    logging.error(f"尝试 {time + 1} 失败: {e}\n{traceback.format_exc()}")
                await asyncio.sleep(time)
            raise Exception(f"尝试 {tries} 次后仍失败")
        return wrapper
    return decorator
