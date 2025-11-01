import asyncio
from datetime import datetime, timedelta
from typing import Any, Dict, List, Optional, Set, cast
from enum import Enum
from . import Amia
from .user import User


class GroupPermission(Enum):
    """群成员权限枚举"""

    OWNER = "owner"  # 群主
    ADMIN = "admin"  # 管理员
    MEMBER = "member"  # 普通成员


class Group:
    """群组类，包含群组的基本信息和操作方法"""

    group_id: int  # 群号
    group_name: str  # 群名称
    member_count: int  # 成员数量
    max_member_count: int  # 最大成员数量
    raw: Dict[str, Any]  # 原始数据
    _last_update_time: datetime  # 最后更新时间

    _instances: Dict[int, "Group"] = {}  # 群组实例缓存

    def __new__(cls, group_id: int, *, bot: Amia) -> "Group":
        """创建Group实例，实现单例模式

        Args:
            group_id: 群组ID
            bot: 机器人实例

        Returns:
            Group: 群组实例
        """
        if group_id not in cls._instances:
            cls._instances[group_id] = super().__new__(cls)
        return cls._instances[group_id]

    @staticmethod
    async def get_group_list(bot: Amia) -> List["Group"]:
        """获取所有群组列表

        Args:
            bot: 机器人实例

        Returns:
            List[Group]: 群组列表
        """
        result = await bot.doAction("get_group_list", {})
        group_list = cast(List[Dict[str, Any]], result.get("data", []))
        groups = [Group(group_id=group["group_id"], bot=bot) for group in group_list]
        await asyncio.gather(*[group.get_info() for group in groups])
        return groups

    @property
    def avatar(self) -> str:
        """获取群头像URL

        Returns:
            str: 群头像URL
        """
        return f"https://p.qlogo.cn/gh/{self.group_id}/{self.group_id}/"

    def __init__(self, group_id: int, *, bot: Amia) -> None:
        """初始化Group实例

        Args:
            group_id: 群组ID
            bot: 机器人实例
        """
        self.group_id = group_id
        self.bot = bot

    async def get_info(self, *, force_update: bool = False) -> "Group":
        """异步获取群组详细信息

        Args:
            force_update: 是否强制更新信息，忽略缓存

        Returns:
            Group: 当前群组实例
        """
        if (
            not force_update
            and hasattr(self, "_last_update_time")
            and datetime.now() - self._last_update_time
            < timedelta(milliseconds=self.bot.config.info_cache_time)
        ):
            return self

        info = await self.bot.doAction("get_group_info", {"group_id": self.group_id})

        data = cast(Dict[str, Any], info.get("data", {}))
        self.raw = data

        # 映射基础属性
        self.group_id = int(data.get("group_id", 0))
        self.group_name = data.get("group_name", "")
        self.member_count = data.get("member_count", 0)
        self.max_member_count = data.get("max_member_count", 0)

        self._last_update_time = datetime.now()

        return self
    
    @classmethod
    async def get_group_list(cls, bot: Amia) -> List["Group"]:
        """获取所有群组列表

        Args:
            bot: 机器人实例

        Returns:
            List[Group]: 群组列表
        """
        result = await bot.doAction("get_group_list", {})
        group_list = cast(List[Dict[str, Any]], result.get("data", []))
        groups = [Group(group_id=group["group_id"], bot=bot) for group in group_list]
        # await asyncio.gather(*[group.get_info() for group in groups])
        return groups

    async def get_member_list_raw(self) -> List[Dict[str, Any]]:
        """获取群成员列表

        Returns:
            List[Dict[str, Any]]: 群成员列表
        """
        result = await self.bot.doAction(
            "get_group_member_list", {"group_id": self.group_id}
        )
        return cast(List[Dict[str, Any]], result.get("data", []))

    async def get_member_list(self) -> List[User]:
        """获取群成员列表

        Returns:
            List[User]: 群成员列表
        """
        member_list_raw = await self.get_member_list_raw()
        return [User(member["user_id"], bot=self.bot) for member in member_list_raw]

    async def get_member_info(self, user_id: int) -> Dict[str, Any]:
        """获取群成员信息

        Args:
            user_id: 用户ID

        Returns:
            Dict[str, Any]: 群成员信息
        """
        result = await self.bot.doAction(
            "get_group_member_info", {"group_id": self.group_id, "user_id": user_id}
        )
        return cast(Dict[str, Any], result.get("data", {}))

    async def get_bot_permission(self) -> GroupPermission:
        """获取机器人在群中的权限

        Returns:
            GroupPermission: 机器人在群中的权限
        """
        bot_user = await self.bot.getBotUser()
        member_info = await self.get_member_info(bot_user.user_id)
        permission = member_info.get("permission", "member")

        try:
            return GroupPermission(permission)
        except ValueError:
            return GroupPermission.MEMBER

    async def set_group_name(self, group_name: str) -> bool:
        """设置群名称

        Args:
            group_name: 新的群名称

        Returns:
            bool: 是否设置成功
        """
        result = await self.bot.doAction(
            "set_group_name", {"group_id": self.group_id, "group_name": group_name}
        )
        # 假设API返回的状态码为0表示成功
        return result.get("status", False) is True

    async def set_group_avatar(self, file_path: str) -> bool:
        """设置群头像

        Args:
            file_path: 头像文件路径

        Returns:
            bool: 是否设置成功
        """
        # 注意: 实际API可能需要不同的参数格式
        result = await self.bot.doAction(
            "set_group_avatar", {"group_id": self.group_id, "file": file_path}
        )
        return result.get("status", False) is True

    async def kick_member(self, user_id: int, reject_add_request: bool = False) -> bool:
        """踢出群成员

        Args:
            user_id: 用户ID
            reject_add_request: 是否拒绝该用户的加群请求

        Returns:
            bool: 是否踢出成功
        """
        result = await self.bot.doAction(
            "set_group_kick",
            {
                "group_id": self.group_id,
                "user_id": user_id,
                "reject_add_request": reject_add_request,
            },
        )
        return result.get("status", False) is True

    async def ban_member(self, user_id: int, duration: int = 60) -> bool:
        """禁言群成员

        Args:
            user_id: 用户ID
            duration: 禁言时长（秒），默认为60秒

        Returns:
            bool: 是否禁言成功
        """
        result = await self.bot.doAction(
            "set_group_ban",
            {"group_id": self.group_id, "user_id": user_id, "duration": duration},
        )
        return result.get("status", False) is True

    async def unban_member(self, user_id: int) -> bool:
        """解除群成员禁言

        Args:
            user_id: 用户ID

        Returns:
            bool: 是否解除禁言成功
        """
        return await self.ban_member(user_id, 0)

    async def set_whole_ban(self, enable: bool) -> bool:
        """设置全体禁言

        Args:
            enable: 是否开启全体禁言

        Returns:
            bool: 是否设置成功
        """
        result = await self.bot.doAction(
            "set_group_whole_ban", {"group_id": self.group_id, "enable": enable}
        )
        return result.get("retcode") == 0

    async def set_admin(self, user_id: int, enable: bool) -> bool:
        """设置群管理员

        Args:
            user_id: 用户ID
            enable: 是否设置为管理员

        Returns:
            bool: 是否设置成功
        """
        result = await self.bot.doAction(
            "set_group_admin",
            {"group_id": self.group_id, "user_id": user_id, "enable": enable},
        )
        return result.get("retcode") == 0

    async def send_announcement(self, content: str, pinned: bool = False) -> bool:
        """发送群公告

        Args:
            content: 公告内容
            pinned: 是否置顶

        Returns:
            bool: 是否发送成功
        """
        result = await self.bot.doAction(
            "_send_group_notice",
            {"group_id": self.group_id, "content": content, "pinned": pinned},
        )
        return result.get("status", False) is True

    async def get_messages_raw(
        self, message_id: int | None = None, count: int | None = None
    ) -> List[Dict[str, Any]]:
        """获取群消息原始数据

        Args:
            message_id: 起始消息ID
            count: 获取消息数量

        Returns:
            List[Dict[str, Any]]: 原始消息数据列表
        """
        params = {"group_id": self.group_id}
        if message_id is not None:
            params["message_id"] = message_id
        if count is not None:
            params["count"] = count
        result = await self.bot.doAction("get_group_msg_history", params)
        if result.get("retcode") == 0:
            return result.get("data", {}).get("messages", [])
        return []

    async def get_messages(self, message_id: int | None = None, count: int | None = None, seq: int | None = None) -> List["RecvMessage"]:  # type: ignore # noqa: F821
        """获取群消息

        Args:
            message_id: 起始消息ID
            count: 获取消息数量，默认为10条

        Returns:
            List[RecvMessage]: 消息列表
        """
        from .recv_message import RecvMessage

        params = {"group_id": self.group_id}
        if message_id is not None:
            params["message_id"] = message_id
        if count is not None:
            params["count"] = count
        if seq is not None:
            params["message_seq"] = seq
        result = await self.bot.doAction("get_group_msg_history", params)
        if result.get("retcode") == 0:
            return [
                RecvMessage.fromDict(msg, self.bot)
                for msg in result.get("data", {}).get("messages", [])
            ]
        return []
    
    async def sign(self):
        """群打卡"""
        return await self.bot.doAction(
            "set_group_sign",
            {"group_id": self.group_id},
        )

    def toDict(self) -> Dict[str, Any]:
        """将群组信息转换为字典

        Returns:
            Dict[str, Any]: 群组信息字典
        """
        return {
            "group_id": self.group_id,
            "group_name": self.group_name,
            "member_count": self.member_count,
            "max_member_count": self.max_member_count,
            "avatar": self.avatar,
        }

    def __str__(self) -> str:
        # 把所有有的信息都包含在字符串中
        info_list = [
            f"群号: {self.group_id}",
            f"群名称: {self.group_name}",
            f"成员数量: {self.member_count}/{self.max_member_count}",
            f"群头像: {self.avatar}",
        ]
        # 把所有有的信息用换行符连接起来
        info_str = "\n".join(info_list)
        return info_str


class GroupMember:
    """群成员类，包含群成员的基本信息和操作方法"""

    user_id: int  # 用户ID
    group_id: int  # 群组ID
    nickname: str  # 昵称
    card: str  # 群名片
    sex: str  # 性别
    age: int  # 年龄
    join_time: datetime  # 入群时间
    last_sent_time: datetime  # 最后发言时间
    level: str  # 群等级
    role: str  # 群角色
    unfriendly: bool  # 是否不良记录
    title: str  # 专属头衔
    title_expire_time: datetime  # 头衔过期时间
    card_changeable: bool  # 是否允许修改群名片
    _last_update_time: datetime  # 最后更新时间

    _instances: Dict[str, "GroupMember"] = {}  # 群成员实例缓存

    def __new__(cls, user_id: int, group_id: int, *, bot: Amia) -> "GroupMember":
        """创建GroupMember实例，实现单例模式

        Args:
            user_id: 用户ID
            group_id: 群组ID
            bot: 机器人实例

        Returns:
            GroupMember: 群成员实例
        """
        key = f"{user_id}_{group_id}"
        if key not in cls._instances:
            cls._instances[key] = super().__new__(cls)
        return cls._instances[key]

    def __init__(self, user_id: int, group_id: int, *, bot: Amia) -> None:
        """初始化GroupMember实例

        Args:
            user_id: 用户ID
            group_id: 群组ID
            bot: 机器人实例
        """
        self.user_id = user_id
        self.group_id = group_id
        self.bot = bot

    async def get_info(self, *, force_update: bool = False) -> "GroupMember":
        """异步获取群成员详细信息

        Args:
            force_update: 是否强制更新信息，忽略缓存

        Returns:
            GroupMember: 当前群成员实例
        """
        if (
            not force_update
            and hasattr(self, "_last_update_time")
            and datetime.now() - self._last_update_time
            < timedelta(milliseconds=self.bot.config.info_cache_time)
        ):
            return self

        info = await self.bot.doAction(
            "get_group_member_info",
            {"group_id": self.group_id, "user_id": self.user_id},
        )

        data = cast(Dict[str, Any], info.get("data", {}))

        # 映射基础属性
        self.user_id = int(data.get("user_id", 0))
        self.group_id = int(data.get("group_id", 0))
        self.nickname = data.get("nickname", "")
        self.card = data.get("card", "")
        self.sex = data.get("sex", "unknown")
        self.age = data.get("age", 0)

        # 处理时间类型数据
        self.join_time = datetime.fromtimestamp(data.get("join_time", 0))
        self.last_sent_time = datetime.fromtimestamp(data.get("last_sent_time", 0))
        self.title_expire_time = datetime.fromtimestamp(
            data.get("title_expire_time", 0)
        )

        # 映射其他属性
        self.level = data.get("level", "")
        self.role = data.get("role", "member")
        self.unfriendly = data.get("unfriendly", False)
        self.title = data.get("title", "")
        self.card_changeable = data.get("card_changeable", True)

        self._last_update_time = datetime.now()

        return self

    @property
    def permission(self) -> GroupPermission:
        """获取群成员权限

        Returns:
            GroupPermission: 群成员权限
        """
        try:
            return GroupPermission(self.role)
        except ValueError:
            return GroupPermission.MEMBER

    async def set_card(self, card: str) -> bool:
        """设置群成员名片

        Args:
            card: 新的群名片

        Returns:
            bool: 是否设置成功
        """
        result = await self.bot.doAction(
            "set_group_card",
            {"group_id": self.group_id, "user_id": self.user_id, "card": card},
        )
        return result.get("status", False) is True

    async def set_title(self, title: str, duration: int = 0) -> bool:
        """设置群成员专属头衔

        Args:
            title: 专属头衔
            duration: 持续时间（秒），0表示永久

        Returns:
            bool: 是否设置成功
        """
        result = await self.bot.doAction(
            "set_group_special_title",
            {
                "group_id": self.group_id,
                "user_id": self.user_id,
                "special_title": title,
                "duration": duration,
            },
        )
        return result.get("status", False) is True

    async def get_user_info(self) -> User:
        """获取用户详细信息

        Returns:
            User: 用户实例
        """
        user = User(self.user_id, bot=self.bot)
        await user.get_info()
        return user

    def toDict(self) -> Dict[str, Any]:
        """将群成员信息转换为字典

        Returns:
            Dict[str, Any]: 群成员信息字典
        """
        return {
            "user_id": self.user_id,
            "group_id": self.group_id,
            "nickname": self.nickname,
            "card": self.card,
            "sex": self.sex,
            "age": self.age,
            "join_time": int(self.join_time.timestamp()),
            "last_sent_time": int(self.last_sent_time.timestamp()),
            "level": self.level,
            "role": self.role,
            "unfriendly": self.unfriendly,
            "title": self.title,
            "title_expire_time": int(self.title_expire_time.timestamp()),
            "card_changeable": self.card_changeable,
            "permission": self.permission.value,
        }
