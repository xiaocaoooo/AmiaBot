from datetime import datetime
import json
import logging
import asyncio
import shutil
import sys
import threading
from pathlib import Path
from amia import Amia
from cache_manager import CacheManager
from config import Config
from openai import OpenAI
from plugin_manager import PluginManager, ProjectInterface
from to_image.html import get_browser_instance


config = Config(Path("./config.json"))

bot = Amia(config.onebot.host, config.onebot.http_port, config.onebot.ws_port, config)  # type: ignore

ProjectInterface().bot = bot

# 配置日志格式
log_format = "[%(asctime)s] [%(levelname)s] [%(filename)s:%(lineno)d] %(message)s"
# 设置日期时间格式
date_format = "%Y-%m-%d %H:%M:%S"

Path("./logs").mkdir(parents=True, exist_ok=True)

OpenAI(config.openai)

# 配置根日志记录器
logging.basicConfig(
    level=logging.INFO,
    format=log_format,
    datefmt=date_format,
    handlers=[
        logging.StreamHandler(sys.stdout),
        logging.FileHandler(f"./logs/{datetime.now().strftime('%Y-%m-%d_%H-%M-%S')}.log")
    ],
)

# 尝试导入WebUI模块
try:
    # from webui.app import run_web_server_async
    from webui.app import run_web_server
    has_webui = True
except ImportError:
    has_webui = False
    logging.warning("WebUI模块导入失败，将仅运行核心功能。")


async def main():
    """主程序入口"""
    # 启动WebUI（如果可用）
    if has_webui:
        try:
            # 在单独的线程中启动WebUI
            webui_thread = threading.Thread(target=run_web_server, daemon=True)  # type: ignore
            webui_thread.start()
            # asyncio.create_task(run_web_server_async()) # type: ignore
            logging.info("WebUI已启动，请访问 http://localhost:5000")
        except Exception as e:
            logging.error(f"启动WebUI失败: {e}")
    else:
        logging.info("WebUI不可用，仅运行核心功能")
    # 初始化插件管理器
    plugin_manager = PluginManager()
    # 加载所有插件
    await plugin_manager.load_all_plugins()
    
    asyncio.create_task(bot.run())
    asyncio.create_task(get_browser_instance())

    # 保持主程序运行
    while True:
        await asyncio.sleep(1)


if __name__ == "__main__":
    # 创建目录
    cache_path = Path("./cache")
    if cache_path.exists():
        shutil.rmtree(cache_path)
    CacheManager(cache_path)
    
    Path("./plugins").mkdir(exist_ok=True)
    Path("./cache/plugins").mkdir(parents=True, exist_ok=True)
    Path("./data/configs/plugins").mkdir(parents=True, exist_ok=True)
    
    group_categories_path = Path("./data/configs/group_categories.json")
    if not group_categories_path.exists():
        group_categories_path.touch()
        with group_categories_path.open("w", encoding="utf-8") as f:
            json.dump([], f, ensure_ascii=False)

    # 运行主程序
    try:
        asyncio.run(main=main())
    except KeyboardInterrupt:
        logging.info("程序已被用户中断")
    except Exception as e:
        logging.error(f"程序运行出错: {e}")
