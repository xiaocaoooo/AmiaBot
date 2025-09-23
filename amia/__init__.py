import json
from typing import Any, Awaitable, Callable, Dict, List, Literal, Optional, cast
import logging
import asyncio
import aiohttp
from config import Config


class Amia:
    _instance: Optional["Amia"] = None

    @staticmethod
    def get_instance() -> "Amia":
        """获取Amia机器人实例的单例

        Returns:
            Amia: Amia机器人实例
        """
        if Amia._instance is None:
            raise ValueError(
                "Amia实例未初始化。请先调用Amia(host, http_port, ws_port, config)。"
            )
        return Amia._instance

    def __init__(self, host: str, http_port: int, ws_port: int, config: Config):
        """初始化Amia机器人实例

        Args:
            host: 服务器主机地址
            http_port: HTTP API端口
            ws_port: WebSocket端口
            config: 配置对象
        """
        if Amia._instance is not None:
            raise ValueError("Amia实例已初始化。请使用Amia.get_instance()获取实例。")
        Amia._instance = self

        self.host = host
        self.http_port = http_port
        self.ws_port = ws_port
        self.config = config
        self._listeners: List[Callable[[Dict[str, Any]], Awaitable[None]]] = []

    async def run(self) -> None:
        """启动WebSocket连接并保持运行

        建立与服务器的WebSocket连接，接收消息并处理。
        连接断开时会自动重试，采用指数退避策略。
        """
        ws_url = f"ws://{self.host}:{self.ws_port}"
        from aiohttp import ClientError

        retry_delay = 1  # 初始重试延迟（秒）
        max_retry_delay = 60  # 最大重试延迟（秒）

        while True:
            try:
                async with aiohttp.ClientSession() as session:
                    async with session.ws_connect(ws_url) as ws:
                        retry_delay = 1  # 成功连接后重置重试延迟
                        async for msg in ws:
                            if msg.type == aiohttp.WSMsgType.TEXT:
                                logging.info(f"Received message: {msg.data}")
                                asyncio.create_task(
                                    self.process_message(json.loads(msg.data))
                                )
                            elif msg.type == aiohttp.WSMsgType.ERROR:
                                logging.error(f"WebSocket错误: {ws.exception()}")
            except (ClientError, asyncio.TimeoutError) as e:
                logging.error(f"连接错误: {str(e)}。{retry_delay}秒后重试...")
                await asyncio.sleep(retry_delay)
                retry_delay = min(retry_delay * 2, max_retry_delay)
            except Exception as e:
                logging.error(f"意外错误: {str(e)}。{retry_delay}秒后重试...")
                await asyncio.sleep(retry_delay)
                retry_delay = min(retry_delay * 2, max_retry_delay)

    def listener(
        self, function: Callable[[Dict[str, Any]], Awaitable[None]]
    ) -> Callable[[Dict[str, Any]], Awaitable[None]]:
        """注册WebSocket消息监听器

        Args:
            function: 接收消息的异步函数，参数为消息字典

        Returns:
            注册的函数本身，便于装饰器使用
        """
        self._listeners.append(function)
        return function

    async def process_message(self, msg: Dict[str, Any]) -> None:
        """处理接收到的WebSocket消息

        Args:
            msg: 消息字典
        """
        await asyncio.gather(*(listener(msg) for listener in self._listeners))

    async def doAction(
        self,
        action: str,
        params: Optional[Dict[str, Any]] = None,
        methods: Literal["GET", "POST"] = "POST",
    ) -> Dict[str, Any]:
        """调用HTTP API执行操作

        Args:
            action: API路径
            params: 请求参数，默认为None
            methods: HTTP方法，可选GET或POST，默认为POST

        Returns:
            API响应结果（JSON解析后的字典）
        """
        logging.info(f"执行操作 {action} {json.dumps(params)}")
        async with aiohttp.ClientSession() as session:
            async with session.request(
                methods,
                f"http://{self.host}:{self.http_port}/{action}",
                json=params or {},
            ) as resp:
                data = cast(Dict[str, Any], await resp.json())
                logging.info(f"操作 {action} 响应: {json.dumps(data)}")
                return data

    async def getBotUser(self) -> "User":  # type: ignore  # noqa: F821
        """获取机器人自身的用户信息

        Returns:
            User对象，包含机器人的详细信息

        Raises:
            ValueError: 获取机器人QQ失败时抛出
        """
        from .user import User

        result = await self.doAction("get_login_info")
        qq = cast(int, result.get("data", {}).get("user_id"))
        if not qq:
            raise ValueError("获取机器人QQ失败")
        user = User(qq, bot=self)
        await user.get_info()
        return user
