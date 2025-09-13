import sys
import os
import logging
import json
from pathlib import Path
import asyncio
from aiohttp import web
from datetime import datetime
import jinja2

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


# 注册路由
app.router.add_get("/", index)
app.router.add_get("/webui/", webui_handler)
app.router.add_get("/api/self", get_self)

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
