import json
from typing import Literal
import logging
import asyncio
import aiohttp


class Amia:
    def __init__(self, host: str, http_port: int, ws_port: int):
        self.host = host
        self.http_port = http_port
        self.ws_port = ws_port
    
    async def run(self):
        """创建ws连接"""
        ws_url = f"ws://{self.host}:{self.ws_port}"
        from aiohttp import ClientError
        retry_delay = 1  # Initial retry delay in seconds
        max_retry_delay = 60  # Maximum retry delay in seconds

        while True:
            try:
                async with aiohttp.ClientSession() as session:
                    async with session.ws_connect(ws_url) as ws:
                        retry_delay = 1  # Reset retry delay after successful connection
                        async for msg in ws:
                            if msg.type == aiohttp.WSMsgType.TEXT:
                                logging.info(f"Received message: {msg.data}")
                                asyncio.create_task(self.process_message(json.loads(msg.data)))
                            elif msg.type == aiohttp.WSMsgType.ERROR:
                                logging.error(f"WebSocket error: {ws.exception()}")
            except (ClientError, asyncio.TimeoutError) as e:
                logging.error(f"Connection error: {str(e)}. Retrying in {retry_delay} seconds...")
                await asyncio.sleep(retry_delay)
                retry_delay = min(retry_delay * 2, max_retry_delay)
            except Exception as e:
                logging.error(f"Unexpected error: {str(e)}. Retrying in {retry_delay} seconds...")
                await asyncio.sleep(retry_delay)
                retry_delay = min(retry_delay * 2, max_retry_delay)
                
    async def process_message(self, msg: dict):
        """处理ws消息"""
        pass

    async def doAction(self, action: str, params: dict|None=None, methods:Literal["GET", "POST"]="POST") -> dict:
        async with aiohttp.ClientSession() as session:
            async with session.request(methods, f"http://{self.host}:{self.http_port}/{action}", json=params or {}) as resp:
                return await resp.json()

    async def getBotUser(self) -> "User": # type: ignore  # noqa: F821
        from .user import User
        qq:int = (await self.doAction("get_login_info")).get("data", {}).get("user_id")
        if not qq:
            raise ValueError("获取机器人QQ失败")
        user= User(qq, bot=self)
        await user.get_info()
        return user
