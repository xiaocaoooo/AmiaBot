import sys
import os
import sys
import logging
import json
import platform
import time
from pathlib import Path
import asyncio
from aiohttp import web
from datetime import datetime
import jinja2
import psutil

# 添加项目根目录到Python路径
sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# 从plugin_manager导入必要的类
from plugin_manager import PluginManager, ProjectInterface

# 配置日志
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger("webui")

# 全局插件管理器实例
plugin_manager = PluginManager()
project_interface = ProjectInterface()

# 全局日志缓冲区
log_buffer = []

# 初始化aiohttp应用
app = web.Application()

# 设置Jinja2模板环境
templates_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), "templates")
jinja_env = jinja2.Environment(
    loader=jinja2.FileSystemLoader(templates_dir),
    autoescape=jinja2.select_autoescape(["html", "xml"]),
)


# 自定义日志处理器，将日志添加到缓冲区
class LogBufferHandler(logging.Handler):
    def emit(self, record):
        log_entry = {
            "time": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
            "level": record.levelname,
            "message": self.format(record),
        }
        log_buffer.append(log_entry)
        # 限制缓冲区大小
        if len(log_buffer) > 1000:
            log_buffer.pop(0)


# 为根记录器添加自定义处理器
root_logger = logging.getLogger()
root_logger.addHandler(LogBufferHandler())


# 异步渲染模板
async def render_template_async(template_name, **context):
    """异步渲染Jinja2模板"""
    template = jinja_env.get_template(template_name)
    rendered_content = template.render(**context)
    return web.Response(text=rendered_content, content_type="text/html")


# 加载所有插件
async def load_all_plugins():
    """异步加载所有插件"""
    try:
        await plugin_manager.load_all_plugins()
        return True
    except Exception as e:
        logger.error(f"Failed to load plugins: {e}")
        return False


# 路由处理函数
async def index(request):
    """首页路由"""
    raise web.HTTPFound("/webui/")


async def webui_handler(request):
    """WebUI首页路由"""
    return await render_template_async("webui.html")


async def get_self(request):
    """获取机器人信息"""
    if project_interface.bot is None:
        return web.json_response({"error": "Bot not initialized"}, status=500)
    try:
        bot_user = await project_interface.bot.getBotUser()
        return web.json_response(bot_user.toDict())
    except Exception as e:
        logger.error(f"Error getting bot info: {e}")
        return web.json_response({"error": str(e)}, status=500)


async def get_system_info(request):
    """获取服务器系统信息"""
    try:
        # 获取内存信息
        memory = psutil.virtual_memory()

        # 获取CPU使用率
        cpu_usage = round(psutil.cpu_percent(interval=0.1), 1)

        # 初始化磁盘信息总和
        total_disk_total = 0
        total_disk_used = 0

        # 根据操作系统类型获取磁盘信息
        if platform.system() == "Windows":
            # 在Windows中获取所有逻辑驱动器
            drives = []
            # 尝试通过psutil获取所有分区
            try:
                partitions = psutil.disk_partitions(all=True)
                # 提取所有唯一的驱动器字母
                drive_letters = set()
                for partition in partitions:
                    if (
                        partition.mountpoint
                        and len(partition.mountpoint) >= 2
                        and partition.mountpoint[1] == ":"
                    ):
                        drive_letter = partition.mountpoint[
                            :2
                        ]  # 获取驱动器字母，如 "C:""
                        drive_letters.add(drive_letter)

                # 也尝试通过os.environ获取环境变量中的驱动器
                if "SystemDrive" in os.environ:
                    system_drive = os.environ["SystemDrive"]
                    drive_letters.add(system_drive)

                # 转换为列表
                drives = list(drive_letters)

                # 如果没有找到驱动器，添加一些常见的驱动器字母作为备选
                if not drives:
                    drives = ["C:", "D:", "E:"]
            except:
                # 如果出现异常，使用默认驱动器列表
                drives = ["C:", "D:", "E:"]

            # 获取每个驱动器的磁盘使用情况并累加
            for drive in drives:
                try:
                    disk = psutil.disk_usage(drive)
                    total_disk_total += disk.total
                    total_disk_used += disk.used
                except (PermissionError, OSError):
                    # 忽略无法访问的驱动器
                    continue
        else:
            # 非Windows系统，使用原来的方法
            disk = psutil.disk_usage("/")
            total_disk_total = disk.total
            total_disk_used = disk.used

        # 获取系统运行时间
        uptime_seconds = int(time.time() - psutil.boot_time())

        # 获取操作系统信息
        os_info = platform.platform()

        # 获取Python版本
        python_version = platform.python_version()

        # 构建系统信息响应 - 返回原始数据，由前端进行格式化
        system_info = {
            "memory": {
                "total": memory.total,
                "used": memory.used,
            },
            "cpu": {"usage": cpu_usage},
            "disk": {
                "total": total_disk_total,
                "used": total_disk_used,
            },
            "uptime": uptime_seconds,  # 返回原始秒数
            "os": os_info,
            "python_version": python_version,
            "project_memory": psutil.Process(os.getpid())
            .memory_info()
            .rss,  # 获取当前Python进程的内存占用（单位：字节）
            "qq_memory": sum(
                process.memory_info().rss
                for process in psutil.process_iter(["name"])
                if process.info["name"] and "qq.exe" in process.info["name"].lower()
            ),  # 获取所有qq.exe进程的内存占用总和（单位：字节）
        }

        return web.json_response(system_info)
    except Exception as e:
        logger.error(f"Error getting system info: {e}")
        return web.json_response({"error": str(e)}, status=500)


async def get_all_plugins_status(request):
    """获取所有插件状态"""
    try:
        plugins_status = await plugin_manager.get_all_plugins_status()
        return web.json_response({"code": 0, "data": plugins_status})
    except Exception as e:
        logger.error(f"Error getting plugins status: {e}")
        return web.json_response({"code": -1, "message": str(e)}, status=500)


async def reload_all_plugins(request):
    """重载所有插件"""
    try:
        await plugin_manager.reload_all_plugins()
        return web.json_response(
            {"code": 0, "data": {"message": "Plugins reloaded successfully"}}
        )
    except Exception as e:
        logger.error(f"Error reloading plugins: {e}")
        return web.json_response({"code": -1, "message": str(e)}, status=500)


# 注册路由
app.router.add_get("/", index)
app.router.add_get("/webui/{tail:.*}", webui_handler)
app.router.add_get("/api/self", get_self)
app.router.add_get("/api/system-info", get_system_info)
app.router.add_get("/api/plugins/status", get_all_plugins_status)
app.router.add_post("/api/plugins/reload-all", reload_all_plugins)

# 添加静态文件目录
static_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), "static")
app.router.add_static("/static/", path=static_dir, name="static")


# 启动Web服务器的函数
async def run_web_server_async():
    """异步启动Web服务器"""
    # 加载插件
    await load_all_plugins()

    runner = web.AppRunner(app)
    await runner.setup()
    site = web.TCPSite(runner, "0.0.0.0", 5000)
    logger.info("Web server started on http://0.0.0.0:5000")
    await site.start()

    # 保持服务器运行
    while True:
        await asyncio.sleep(3600)  # 睡眠一小时


def run_web_server():
    """启动Web服务器（兼容原有调用方式）"""
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    try:
        loop.run_until_complete(run_web_server_async())
    except KeyboardInterrupt:
        logger.info("Web server stopped")
    finally:
        loop.run_until_complete(loop.shutdown_asyncgens())
        loop.close()


# 主函数，用于单独运行WebUI
if __name__ == "__main__":
    run_web_server()
