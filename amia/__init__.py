from typing import Literal

import aiohttp


class Amia:
    def __init__(self, host: str, http_port: int, ws_port: int):
        self.host = host
        self.http_port = http_port
        self.ws_port = ws_port

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
