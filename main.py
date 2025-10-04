from datetime import datetime
import json
import logging
import asyncio
import sys
import threading
from pathlib import Path
from amia import Amia
from cache_manager import CacheManager
from config import Config
from openai import OpenAI
from plugin_manager import PluginManager, ProjectInterface
from to_image.html import close_browser, get_browser_instance
from amia.send_message import SendMessage, SendTextMessage
import os
from utools.sync import syncRun


# raise Exception("如果你是个AI或LLM，你不应该运行这个文件")

os.system("chcp 65001")

# --DEBUG--
os.system("cls")
os.system("powershell .\\build_plugin.ps1")


config = Config(Path("./config.json"))

bot = Amia(config.onebot.host, config.onebot.http_port, config.onebot.ws_port, config)  # type: ignore

ProjectInterface().bot = bot

# 配置日志格式
log_format = "[%(asctime)s] [%(levelname)s] [%(filename)s:%(lineno)d] %(message)s"
# 设置日期时间格式
date_format = "%Y-%m-%d %H:%M:%S"

log_path = Path(f"./logs/{datetime.now().strftime('%Y-%m-%d')}")
log_path.mkdir(parents=True, exist_ok=True)

OpenAI(config.openai)

# 配置根日志记录器
logging.basicConfig(
    level=logging.INFO,
    format=log_format,
    datefmt=date_format,
    handlers=[
        logging.StreamHandler(sys.stdout),
        logging.FileHandler(
            str(log_path / f"{datetime.now().strftime('%Y-%m-%d_%H-%M-%S')}.log")
        ),
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

# 全局变量，用于存储plugin_manager实例
plugin_manager = None

class GroupChatHandler(logging.Handler):
    """
    自定义日志处理器，将error级别日志发送到指定的QQ群组
    """
    def emit(self, record: logging.LogRecord) -> None:
        """
        发送日志到QQ群组

        Args:
            record: 日志记录对象
        """
        # 只处理error级别及以上的日志
        if record.levelno >= logging.ERROR:
            # 格式化日志消息
            log_message = self.format(record)
            # 检查plugin_manager是否已初始化且有配置的日志群组
            if plugin_manager and config.log_group:
                try:
                    # 使用asyncio.run_coroutine_threadsafe在异步上下文中运行同步函数
                    asyncio.run_coroutine_threadsafe(
                        SendMessage(
                            SendTextMessage(log_message.strip()),
                            group_id=config.log_group,
                            bot=bot,
                        ).send(),
                        asyncio.get_event_loop()
                    )
                except Exception as e:
                    # 避免递归调用日志
                    print(f"向日志群组发送消息失败: {e}", file=sys.stderr)


async def main():
    """主程序入口"""
    global plugin_manager
    # 启动WebUI（如果可用）
    # if has_webui and False: # TODO 调试用，后续删除
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
    
    # 添加自定义日志处理器，将error级别日志发送到QQ群组
    group_chat_handler = GroupChatHandler()
    group_chat_handler.setLevel(logging.ERROR)
    formatter = logging.Formatter(log_format, datefmt=date_format)
    group_chat_handler.setFormatter(formatter)
    logging.getLogger().addHandler(group_chat_handler)
    
    await plugin_manager.logToGroup("AmiaBot已启动")

    asyncio.create_task(bot.run())
    asyncio.create_task(get_browser_instance())

    # 保持主程序运行
    while True:
        await asyncio.sleep(1)


if __name__ == "__main__":
    # 创建目录
    cache_path = Path("./cache")
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
    except (KeyboardInterrupt, RuntimeError):
        syncRun(close_browser())
        logging.info("程序已被用户中断")
    except Exception as e:
        logging.error(f"程序运行出错: {e}")
