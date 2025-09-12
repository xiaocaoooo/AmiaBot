import asyncio
import logging
from pathlib import Path
import sys
from plugin_manager import PluginManager


async def main():
    plugin_manager = PluginManager()
    await plugin_manager.load_all_plugins()


if __name__ == "__main__":
    # 配置日志格式
    log_format = '[%(asctime)s] [%(levelname)s] [%(filename)s:%(lineno)d] %(message)s'
    # 设置日期时间格式
    date_format = '%Y-%m-%d %H:%M:%S'

    # 配置根日志记录器
    logging.basicConfig(
        level=logging.INFO,
        format=log_format,
        datefmt=date_format,
        handlers=[
            logging.StreamHandler(sys.stdout),
            # 如果需要文件日志，可以添加下面这行
            # logging.FileHandler('app.log')
        ]
    )
    
    # 创建目录
    Path("./plugins").mkdir(exist_ok=True)
    Path("./cache/plugins").mkdir(parents=True, exist_ok=True)

    # 运行主程序
    asyncio.run(main())
