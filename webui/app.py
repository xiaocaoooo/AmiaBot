import sys
import os
import logging
import platform
import time
import asyncio
import json
import hashlib
import uuid
from datetime import datetime, timedelta
from aiohttp import web
import jinja2
import psutil
from typing import Dict, Any, List, Optional
from pathlib import Path

# 添加项目根目录到Python路径
sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# 从plugin_manager导入必要的类
from plugin_manager import PluginManager, ProjectInterface
from config import Config

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

# 读取配置文件
def load_config() -> Config:
    """加载配置文件"""
    config_path = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "config.json")
    return Config(Path(config_path))

# 全局配置对象
config = load_config()

# 全局会话字典，用于存储登录会话
# 键为会话ID，值为(过期时间, 用户信息)
sessions = {}

# 生成会话ID
def generate_session_id() -> str:
    """生成唯一的会话ID"""
    return str(uuid.uuid4())

# 验证密码
def verify_password(input_password: str) -> bool:
    """验证用户输入的密码是否正确"""
    try:
        # 从配置中获取密码
        correct_password = config.webui.password # type: ignore
        
        # 检查是否是哈希密码（长度为64个字符的十六进制字符串）
        if len(input_password) == 64 and all(c in '0123456789abcdefABCDEF' for c in input_password):
            # 计算正确密码的哈希值
            correct_password_hash = hashlib.sha256(correct_password.encode()).hexdigest()
            return input_password.lower() == correct_password_hash.lower()
        else:
            # 否则，直接比较明文密码（向后兼容）
            return input_password == correct_password
    except Exception as e:
        logger.error(f"Error verifying password: {e}")
        return False

# 认证中间件
@web.middleware
async def auth_middleware(request: web.Request, handler):
    """认证中间件，用于验证用户登录状态"""
    # 排除登录相关路由
    if request.path == "/login" or request.path == "/":
        return await handler(request)
    
    # 获取会话ID
    session_id = request.cookies.get("session_id")
    
    # 验证会话ID
    if not session_id or session_id not in sessions:
        # 未登录或会话不存在，重定向到登录页
        raise web.HTTPFound("/login")
    
    # 检查会话是否过期
    expiry_time = sessions[session_id][0]
    if datetime.now() > expiry_time:
        # 会话过期，删除会话并重定向到登录页
        del sessions[session_id]
        raise web.HTTPFound("/login")
    
    # 会话有效，继续处理请求
    return await handler(request)

# 设置Jinja2模板环境
templates_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), "templates")
jinja_env = jinja2.Environment(
    loader=jinja2.FileSystemLoader(templates_dir),
    autoescape=jinja2.select_autoescape(["html", "xml"]),
)

# 添加认证中间件到应用
app.middlewares.append(auth_middleware)


# 自定义日志处理器，将日志添加到缓冲区
class LogBufferHandler(logging.Handler):
    def emit(self, record):
        log_entry = {
            "time": time.time(),
            "level": record.levelname,
            "message": self.format(record),
            "module": record.module,
            "function": record.funcName,
            "line": record.lineno,
            "process": record.process,
            "thread": record.thread,
            "process_name": record.processName,
            "thread_name": record.threadName,
        }
        log_buffer.append(log_entry)
        # Limit the buffer size
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
    raise web.HTTPFound("/login")


async def login(request):
    """登录路由处理函数"""
    if request.method == "GET":
        # GET请求，显示登录页面
        return await render_template_async("login.html")
    elif request.method == "POST":
        # POST请求，处理登录表单提交
        form = await request.post()
        
        # 优先使用哈希密码
        password = form.get("hashed_password", "")
        
        # 如果没有哈希密码，则使用明文密码（向后兼容）
        if not password:
            password = form.get("password", "")
        
        # 验证密码
        if verify_password(password):
            # 密码正确，生成会话ID
            session_id = generate_session_id()
            # 设置会话过期时间为2小时
            expiry_time = datetime.now() + timedelta(hours=2)
            # 存储会话信息
            sessions[session_id] = (expiry_time, {"user": "admin"})
            
            # 创建响应对象
            response = web.HTTPFound("/webui/")
            # 设置Cookie，过期时间与会话一致
            response.set_cookie(
                "session_id", 
                session_id, 
                expires=expiry_time.timestamp(), # type: ignore
                httponly=True,  # 设置httponly，提高安全性
                samesite="Strict"  # 设置samesite，防止CSRF攻击
            )
            return response
        else:
            # 密码错误，重新显示登录页面并提示错误
            return await render_template_async("login.html", error="密码错误，请重新输入")
    
    # 不支持的请求方法
    return web.Response(text="Method not allowed", status=405)


async def webui_handler(request):
    """WebUI首页路由"""
    # 检查登录状态（已在中间件中完成）
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
            except Exception:
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


async def reload_plugin(request):
    """重载指定插件"""
    try:
        data = await request.json()
        plugin_id = data.get("plugin_id")
        if not plugin_id:
            return web.json_response(
                {"code": -1, "message": "Plugin ID is required"}, status=400
            )
        await plugin_manager.reload_plugin(plugin_id)
        return web.json_response(
            {
                "code": 0,
                "data": {"message": f"Plugin {plugin_id} reloaded successfully"},
            }
        )
    except Exception as e:
        # Get plugin_id from local scope in case of error
        plugin_id = data.get("plugin_id") if "data" in locals() else "unknown"  # type: ignore
        logger.error(f"Error reloading plugin {plugin_id}: {e}")
        return web.json_response({"code": -1, "message": str(e)}, status=500)


async def enable_plugin(request):
    """启用指定插件"""
    try:
        data = await request.json()
        plugin_id = data.get("plugin_id")
        if not plugin_id:
            return web.json_response(
                {"code": -1, "message": "Plugin ID is required"}, status=400
            )
        enabled = await plugin_manager.enable_plugin(plugin_id)
        if enabled:
            return web.json_response(
                {"code": 0, "data": {"message": f"Plugin {plugin_id} enabled successfully"}}
            )
        else:
            return web.json_response(
                {"code": -1, "message": f"Failed to enable plugin {plugin_id}"}, status=500
            )
    except Exception as e:
        # Get plugin_id from local scope in case of error
        plugin_id = data.get("plugin_id") if "data" in locals() else "unknown"  # type: ignore
        logger.error(f"Error enabling plugin {plugin_id}: {e}")
        return web.json_response({"code": -1, "message": str(e)}, status=500)


async def disable_plugin(request):
    """禁用指定插件"""
    try:
        data = await request.json()
        plugin_id = data.get("plugin_id")
        if not plugin_id:
            return web.json_response(
                {"code": -1, "message": "Plugin ID is required"}, status=400
            )
        disabled = await plugin_manager.disable_plugin(plugin_id)
        if disabled:
            return web.json_response(
                {
                    "code": 0,
                    "data": {"message": f"Plugin {plugin_id} disabled successfully"},
                }
            )
        else:
            return web.json_response(
                {"code": -1, "message": f"Failed to disable plugin {plugin_id}"}, status=500
            )
    except Exception as e:
        # Get plugin_id from local scope in case of error
        plugin_id = data.get("plugin_id") if "data" in locals() else "unknown"  # type: ignore
        logger.error(f"Error disabling plugin {plugin_id}: {e}")
        return web.json_response({"code": -1, "message": str(e)}, status=500)


async def get_logs(request):
    """分页获取日志信息
    
    Args:
        request: HTTP请求对象，包含查询参数
            - page: 当前页码，默认为1
            - page_size: 每页记录数，默认为20
    
    Returns:
        JSON响应，包含分页日志数据和元信息
    """
    try:
        # 获取查询参数
        page = int(request.query.get("page", 1))
        page_size = int(request.query.get("page_size", 20))
        
        # 参数验证
        if page < 1:
            page = 1
        if page_size < 1 or page_size > 100:
            page_size = 20
        
        # 计算总页数
        total_logs = len(log_buffer)
        total_pages = (total_logs + page_size - 1) // page_size
        
        # 计算当前页的起始和结束索引
        start_index = (page - 1) * page_size
        end_index = start_index + page_size
        
        # 获取当前页的日志数据
        current_page_logs = log_buffer[start_index:end_index]
        
        # 构建响应数据
        response_data = {
            "code": 0,
            "data": {
                "logs": current_page_logs,
                "pagination": {
                    "current_page": page,
                    "page_size": page_size,
                    "total_pages": total_pages,
                    "total_logs": total_logs
                }
            }
        }
        
        return web.json_response(response_data)
    except Exception as e:
        logger.error(f"Error getting logs: {e}")
        return web.json_response({"code": -1, "message": str(e)}, status=500)


# 注册路由
app.router.add_get("/", index)
app.router.add_route("*", "/login", login)  # 支持GET和POST
app.router.add_get("/webui/{tail:.*}", webui_handler)
app.router.add_get("/api/self", get_self)
app.router.add_get("/api/system-info", get_system_info)
app.router.add_get("/api/plugins/status", get_all_plugins_status)
app.router.add_post("/api/plugins/reload-all", reload_all_plugins)
app.router.add_post("/api/plugins/reload", reload_plugin)
app.router.add_post("/api/plugins/enable", enable_plugin)
app.router.add_post("/api/plugins/disable", disable_plugin)
app.router.add_get("/api/logs", get_logs)

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
