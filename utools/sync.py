import asyncio
import logging
import threading
import traceback
from typing import Any, Awaitable


def syncRun(future):
    """同步运行异步函数

    Args:
        future: 要运行的异步函数

    Returns:
        异步函数的执行结果
    """
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    res = None
    try:
        res = loop.run_until_complete(future)
    except Exception as e:
        logging.error(f"同步运行异步函数失败: {e}\n{traceback.format_exc()}")
    finally:
        loop.close()
        return res


def syncRunWithNewThread(future):
    """同步运行异步函数，在新线程中执行

    Args:
        future: 要运行的异步函数

    Returns:
        异步函数的执行结果
    """
    result = None

    def run_and_set_result():
        nonlocal result
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            result = loop.run_until_complete(future)
        except Exception as e:
            logging.error(f"同步运行异步函数失败: {e}\n{traceback.format_exc()}")
        finally:
            loop.close()

    thread = threading.Thread(target=run_and_set_result)
    thread.start()
    thread.join()
    return result


async def asyncRunWithNewThread(future: Awaitable[Any]) -> Any:
    """在新线程中异步运行异步函数。

    Args:
        future: 要运行的异步函数。

    Returns:
        异步函数的执行结果。
    """
    return await asyncio.to_thread(syncRunWithNewThread, future)
