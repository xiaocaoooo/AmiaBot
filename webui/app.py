import sys
import os
import json
import logging
import platform
import time
import asyncio
import hashlib
import uuid
from datetime import datetime, timedelta
from aiohttp import web
import jinja2
import psutil
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
    config_path = os.path.join(
        os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "config.json"
    )
    return Config(Path(config_path))


# 全局配置对象
config = load_config()

# 全局会话字典，用于存储登录会话
# 键为会话ID，值为(过期时间, 用户信息)
sessions = {}

# 全局存储失败登录尝试的字典
# 键为客户端IP，值为(尝试次数, 首次尝试时间)
failed_login_attempts = {}


# 生成会话ID
def generate_session_id() -> str:
    """生成唯一的会话ID"""
    return str(uuid.uuid4())


# 验证密码
def verify_password(input_password: str) -> bool:
    """验证用户输入的密码是否正确"""
    try:
        # 从配置中获取密码
        correct_password = config.webui.password  # type: ignore

        # 计算正确密码的哈希值
        correct_password_hash = hashlib.sha256(correct_password.encode()).hexdigest()
        return input_password.lower() == correct_password_hash.lower()
    except Exception as e:
        logger.error(f"Error verifying password: {e}")
        return False


# 认证中间件
@web.middleware
async def auth_middleware(request: web.Request, handler):
    """认证中间件，用于验证用户登录状态"""
    # 排除登录相关路由
    if (
        request.path == "/login"
        or request.path == "/"
        or request.path.startswith("/static")
        or request.path.startswith("/fonts")
        or request.path.startswith("/favicon.ico")
    ):
        return await handler(request)

    # 获取会话ID
    session_id = request.cookies.get("session_id")

    # 验证会话ID
    if not session_id or session_id not in sessions:
        # 未登录或会话不存在，重定向到登录页
        raise web.HTTPFound(f"/login?url={request.path}")

    # 检查会话是否过期
    expiry_time = sessions[session_id][0]
    if datetime.now() > expiry_time:
        # 会话过期，删除会话并重定向到登录页
        del sessions[session_id]
        raise web.HTTPFound(f"/login?url={request.path}")

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
        # 获取重定向URL参数
        url = request.query.get("url", "/webui/")
        # 显示登录页面并传递url参数
        return await render_template_async("login.html", url=url)
    elif request.method == "POST":
        # POST请求，处理登录表单提交
        form = await request.post()

        # 获取客户端IP
        client_ip = request.remote or "unknown"

        # 检查失败次数
        if client_ip in failed_login_attempts:
            attempts, timestamp = failed_login_attempts[client_ip]
            # 5分钟内超过5次失败尝试，则暂时拒绝登录
            if attempts >= 5 and (datetime.now() - timestamp).seconds < 300:
                return await render_template_async(
                    "login.html", error="登录失败次数过多，请稍后再试"
                )

        # 优先使用哈希密码
        password = form.get("hashed_password", "")

        # 如果没有哈希密码，则使用明文密码
        if not password:
            password = form.get("password", "")

        # 获取重定向URL
        redirect_url = form.get("url", "/webui/")
        # 验证URL安全性，防止开放重定向攻击
        if not redirect_url.startswith("/"):
            redirect_url = "/webui/"

        # 验证密码
        if verify_password(password):
            # 登录成功，清除失败记录
            if client_ip in failed_login_attempts:
                del failed_login_attempts[client_ip]

            # 密码正确，生成会话ID
            session_id = generate_session_id()

            # 获取记住我选项，决定会话过期时间
            remember_me = form.get("remember_me", "")
            expiry_time = datetime.now() + timedelta(
                hours=2 if not remember_me else 7 * 24
            )

            # 存储会话信息
            sessions[session_id] = (expiry_time, {"user": "admin"})

            # 重定向到指定URL
            response = web.HTTPFound(redirect_url)

            # 设置Cookie，过期时间与会话一致
            response.set_cookie(
                "session_id",
                session_id,
                expires=expiry_time.timestamp(),  # type: ignore
                httponly=True,  # 设置httponly，提高安全性
                samesite="Strict",  # 设置samesite，防止CSRF攻击
                secure=request.scheme == "https",  # 条件性设置secure属性
            )
            return response
        else:
            # 记录失败尝试
            if (
                client_ip not in failed_login_attempts
                or (datetime.now() - failed_login_attempts[client_ip][1]).seconds > 300
            ):
                failed_login_attempts[client_ip] = (1, datetime.now())
            else:
                attempts, timestamp = failed_login_attempts[client_ip]
                failed_login_attempts[client_ip] = (attempts + 1, timestamp)

            # 密码错误，重新显示登录页面并提示错误
            return await render_template_async(
                "login.html", error="密码错误，请重新输入", url=redirect_url
            )

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


async def get_groups(request):
    """获取群聊列表"""
    if project_interface.bot is None:
        return web.json_response({"error": "Bot not initialized"}, status=500)
    try:
        from amia.group import Group

        groups = await Group.get_group_list(project_interface.bot)
        groups_info = [group.toDict() for group in groups]
        return web.json_response(groups_info)
    except Exception as e:
        logger.error(f"Error getting groups list: {e}")
        return web.json_response({"error": str(e)}, status=500)


async def get_group_info(request):
    """获取特定群聊信息"""
    if project_interface.bot is None:
        return web.json_response({"error": "Bot not initialized"}, status=500)
    try:
        from amia.group import Group

        if "group_id" not in request.query:
            return web.json_response(
                {"error": "Missing group_id parameter"}, status=400
            )
        group_id = int(request.query.get("group_id", 0))
        group = Group(group_id, bot=project_interface.bot)
        await group.get_info()
        return web.json_response(group.toDict())
    except Exception as e:
        logger.error(f"Error getting group info: {e}")
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
                {
                    "code": 0,
                    "data": {"message": f"Plugin {plugin_id} enabled successfully"},
                }
            )
        else:
            return web.json_response(
                {"code": -1, "message": f"Failed to enable plugin {plugin_id}"},
                status=500,
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
                {"code": -1, "message": f"Failed to disable plugin {plugin_id}"},
                status=500,
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
        no_webui = request.query.get("no_webui", "false").lower() == "true"
        only_message = request.query.get("only_message", "false").lower() == "true"
        log_buffer_copy = log_buffer.copy()
        log_buffer_copy.reverse()
        if no_webui:
            log_buffer_copy = [
                log for log in log_buffer_copy if "web_log" not in log.get("module", "")
            ]
        if only_message:
            log_buffer_copy = [
                log
                for log in log_buffer_copy
                if log.get("message", "").startswith("Received message: ")
            ]

        # 参数验证
        if page < 1:
            page = 1
        if page_size < 1 or page_size > 100:
            page_size = 20

        # 计算总页数
        total_logs = len(log_buffer_copy)
        total_pages = (total_logs + page_size - 1) // page_size

        # 计算当前页的起始和结束索引
        start_index = (page - 1) * page_size
        end_index = start_index + page_size

        # 获取当前页的日志数据
        current_page_logs = log_buffer_copy[start_index:end_index]

        # 构建响应数据
        response_data = {
            "code": 0,
            "data": {
                "logs": current_page_logs,
                "pagination": {
                    "current_page": page,
                    "page_size": page_size,
                    "total_pages": total_pages,
                    "total_logs": total_logs,
                },
            },
        }

        return web.json_response(response_data)
    except Exception as e:
        logger.error(f"Error getting logs: {e}")
        return web.json_response({"code": -1, "message": str(e)}, status=500)


async def font_handler(request):
    """处理字体文件请求"""
    try:
        font_path = str(request.url).split("/")[-1]
        if not font_path:
            return web.json_response(
                {"code": -1, "message": "Font path is required"}, status=400
            )
        font_path = os.path.join(os.path.dirname(__file__), "..", "fonts", font_path)
        if not os.path.exists(font_path):
            return web.json_response(
                {"code": -1, "message": "Font file not found"}, status=404
            )
        with open(font_path, "rb") as f:
            font_data = f.read()
        return web.Response(body=font_data, content_type="font/ttf")
    except Exception as e:
        logger.error(f"Error serving font file: {e}")
        return web.json_response({"code": -1, "message": str(e)}, status=500)


async def favicon_handler(request):
    """处理favicon.ico请求"""
    # logo.svg
    with open(os.path.join(os.path.dirname(__file__), "..", "logo.svg"), "rb") as f:
        logo_data = f.read()
    return web.Response(body=logo_data, content_type="image/svg+xml")


async def get_group_categories(request):
    """获取群组分类配置

    Returns:
        JSON响应，包含群组分类配置数据
    """
    try:
        # 获取group_categories.json文件路径
        file_path = os.path.join(
            os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
            "data",
            "configs",
            "group_categories.json",
        )

        # 检查文件是否存在
        if not os.path.exists(file_path):
            return web.json_response(
                {"code": -1, "message": "Group categories file not found"}, status=404
            )

        # 读取并解析JSON文件
        with open(file_path, "r", encoding="utf-8") as f:
            categories_data = json.load(f)
        
        # 转换为数组格式以便前端处理
        if isinstance(categories_data, dict):
            categories_data = list(categories_data.values())

        # 返回配置数据
        return web.json_response({"code": 0, "data": categories_data})
    except json.JSONDecodeError as e:
        logger.error(f"Error parsing group categories file: {e}")
        return web.json_response(
            {"code": -1, "message": f"Invalid JSON format: {str(e)}"}, status=400
        )
    except Exception as e:
        logger.error(f"Error reading group categories: {e}")
        return web.json_response({"code": -1, "message": str(e)}, status=500)


async def update_group_categories(request):
    """更新群组分类配置

    Args:
        request: HTTP请求对象，包含JSON数据
            - data: 群组分类配置数据

    Returns:
        JSON响应，指示操作结果
    """
    try:
        # 获取请求数据
        request_data = await request.json()
        categories_data = request_data.get("data")

        # 验证请求数据
        if categories_data is None:
            return web.json_response(
                {"code": -1, "message": "Categories data is required"}, status=400
            )

        # 获取group_categories.json文件路径
        file_path = os.path.join(
            os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
            "data",
            "configs",
            "group_categories.json",
        )

        # 确保目录存在
        os.makedirs(os.path.dirname(file_path), exist_ok=True)

        # 将数组转换为对象格式（使用id作为键）
        if isinstance(categories_data, list):
            categories_dict = {}
            for category in categories_data:
                if "id" in category:
                    categories_dict[category["id"]] = category
            categories_data = categories_dict
        
        # 写入格式化的JSON内容
        with open(file_path, "w", encoding="utf-8") as f:
            json.dump(categories_data, f, ensure_ascii=False, indent=2)

        # 返回成功响应
        return web.json_response(
            {"code": 0, "data": {"message": "Group categories updated successfully"}}
        )
    except Exception as e:
        logger.error(f"Error updating group categories: {e}")
        return web.json_response({"code": -1, "message": str(e)}, status=500)


async def get_plugin_config(request):
    """获取插件配置

    Args:
        request: HTTP请求对象，包含查询参数
            - id: 插件ID

    Returns:
        JSON响应，包含插件配置数据
    """
    try:
        # 获取插件ID
        plugin_id = request.query.get("id")
        if not plugin_id:
            return web.json_response(
                {"code": -1, "message": "Plugin ID is required"}, status=400
            )

        # 获取插件配置文件路径
        file_path = os.path.join(
            os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
            "data",
            "configs",
            "plugins",
            f"{plugin_id}.json",
        )

        # 检查文件是否存在
        if not os.path.exists(file_path):
            return web.json_response(
                {"code": -1, "message": "Plugin config file not found"}, status=404
            )

        # 读取并解析JSON文件
        with open(file_path, "r", encoding="utf-8") as f:
            config_data = json.load(f)

        # 返回配置数据
        return web.json_response({"code": 0, "data": config_data})
    except json.JSONDecodeError as e:
        logger.error(f"Error parsing plugin config file: {e}")
        return web.json_response(
            {"code": -1, "message": f"Invalid JSON format: {str(e)}"}, status=400
        )
    except Exception as e:
        logger.error(f"Error reading plugin config: {e}")
        return web.json_response({"code": -1, "message": str(e)}, status=500)


async def update_plugin_config(request):
    """更新插件配置

    Args:
        request: HTTP请求对象，包含JSON数据
            - plugin_id: 插件ID
            - config: 插件配置数据

    Returns:
        JSON响应，指示操作结果
    """
    try:
        # 获取请求数据
        request_data = await request.json()
        plugin_id = request_data.get("plugin_id")
        config_data = request_data.get("config")

        # 验证请求数据
        if not plugin_id:
            return web.json_response(
                {"code": -1, "message": "Plugin ID is required"}, status=400
            )
        if config_data is None:
            return web.json_response(
                {"code": -1, "message": "Config data is required"}, status=400
            )

        # 获取插件配置文件路径
        file_path = os.path.join(
            os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
            "data",
            "configs",
            "plugins",
            f"{plugin_id}.json",
        )

        # 确保目录存在
        os.makedirs(os.path.dirname(file_path), exist_ok=True)

        # 写入格式化的JSON内容
        with open(file_path, "w", encoding="utf-8") as f:
            json.dump(config_data, f, ensure_ascii=False, indent=2)

        # 返回成功响应
        return web.json_response(
            {
                "code": 0,
                "data": {"message": f"Plugin {plugin_id} config updated successfully"},
            }
        )
    except Exception as e:
        logger.error(f"Error updating plugin config: {e}")
        return web.json_response({"code": -1, "message": str(e)}, status=500)


# 注册路由
app.router.add_get("/", index)
app.router.add_route("*", "/login", login)
app.router.add_get("/fonts/{font:.*}", font_handler)
app.router.add_get("/favicon.ico", favicon_handler)
app.router.add_get("/webui/{tail:.*}", webui_handler)
app.router.add_get("/api/self", get_self)
app.router.add_get("/api/system-info", get_system_info)
app.router.add_get("/api/plugins/status", get_all_plugins_status)
app.router.add_post("/api/plugins/reload-all", reload_all_plugins)
app.router.add_post("/api/plugins/reload", reload_plugin)
app.router.add_post("/api/plugins/enable", enable_plugin)
app.router.add_post("/api/plugins/disable", disable_plugin)
app.router.add_get("/api/logs", get_logs)
# 添加专门的群组分类API路由
app.router.add_get("/api/group-categories/get", get_group_categories)
app.router.add_post("/api/group-categories/set", update_group_categories)
# 添加插件配置API路由
app.router.add_get("/api/plugin-config/get", get_plugin_config)
app.router.add_post("/api/plugin-config/set", update_plugin_config)
# 添加群聊相关API路由
app.router.add_get("/api/group/list", get_groups)
app.router.add_get("/api/group/get", get_group_info)

# 添加静态文件目录
static_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), "static")
app.router.add_static("/static/", path=static_dir, name="static")


# 启动Web服务器的函数
async def cleanup_sessions():
    """定期清理过期会话"""
    while True:
        current_time = datetime.now()
        expired_sessions = [
            sid for sid, (expiry, _) in sessions.items() if current_time > expiry
        ]
        for sid in expired_sessions:
            del sessions[sid]
        await asyncio.sleep(3600)  # 每小时清理一次


async def run_web_server_async():
    """异步启动Web服务器"""
    # 启动会话清理任务
    asyncio.create_task(cleanup_sessions())

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
