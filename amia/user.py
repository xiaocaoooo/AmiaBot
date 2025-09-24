from enum import Enum
from datetime import datetime, timedelta
from typing import Any, Dict, Optional, cast
from . import Amia


class Sex(Enum):
    """用户性别枚举"""

    UNKNOWN = "unknown"  # 未知性别
    FEMALE = "female"  # 女性
    MALE = "male"  # 男性


class GroupRole(Enum):
    """群成员角色枚举"""

    UNKNOWN = "unknown"  # 未知角色
    OWNER = "owner"  # 群主
    ADMIN = "admin"  # 管理员
    MEMBER = "member"  # 普通成员
    GUEST = "guest"  # 访客（某些群可能有此角色）

    @classmethod
    def _missing_(cls, value):
        """处理未在枚举中定义的角色值"""
        # 如果值不是预定义的角色，返回UNKNOWN
        return cls.UNKNOWN


class User:
    """用户类，包含用户的基本信息和操作方法"""

    qq: int  # QQ号码
    nick: str  # 昵称
    remark: str  # 备注
    country: str  # 国家
    city: str  # 城市
    reg_time: datetime  # 注册时间
    qid: str  # QID
    birthday: Optional[datetime]  # 生日
    age: int  # 年龄
    sex: Sex  # 性别
    _last_update_time: datetime  # 最后更新时间
    # 群成员相关属性
    user_id: int  # 用户ID
    group_id: Optional[int]  # 群组ID
    card: Optional[str]  # 群名片
    role: Optional[GroupRole]  # 群内角色
    group_level: Optional[str]  # 群等级
    shut_up_timestamp: Optional[int]  # 禁言时间戳
    join_time: Optional[datetime]  # 加入群时间

    _instances: Dict[str, "User"] = {}  # 用户实例缓存

    def __new__(
        cls, user_id: int, *, group_id: Optional[int] = None, bot: Amia
    ) -> "User":
        """创建User实例，实现单例模式

        Args:
            user_id: 用户ID
            group_id: 群组ID（可选）
            bot: 机器人实例

        Returns:
            User: 用户实例
        """
        key = f"{user_id}_{group_id}"
        if key not in cls._instances:
            cls._instances[key] = super().__new__(cls)
        return cls._instances[key]

    @property
    def avatar(self) -> str:
        """获取用户头像URL

        Returns:
            str: 头像URL
        """
        return f"https://q1.qlogo.cn/g?b=qq&nk={self.qq}&s=0"

    def __init__(
        self, user_id: int, *, group_id: Optional[int] = None, bot: Amia
    ) -> None:
        """初始化User实例

        Args:
            user_id: 用户ID
            group_id: 群组ID（可选）
            bot: 机器人实例
        """
        self.user_id = user_id
        self.group_id = group_id
        self.bot = bot

    async def get_info(self, *, force_update: bool = False) -> "User":
        """异步获取用户详细信息

        Args:
            force_update: 是否强制更新信息，忽略缓存

        Returns:
            User: 当前用户实例
        """
        if (
            not force_update
            and hasattr(self, "_last_update_time")
            and datetime.now() - self._last_update_time
            < timedelta(milliseconds=self.bot.config.info_cache_time)
        ):
            return self

        info = await self.bot.doAction("get_stranger_info", {"user_id": self.user_id})

        data = cast(Dict[str, Any], info.get("data", {}))

        # 映射基础属性
        self.qq = int(data.get("uin", 0))
        self.nick = data.get("nick", "")
        self.remark = data.get("remark", "")
        self.country = data.get("country", "")
        self.city = data.get("city", "")

        # 处理时间类型数据
        self.reg_time = datetime.fromtimestamp(data.get("regTime", 0))

        # 处理生日日期
        self.birthday = (
            datetime(
                year=data.get("birthday_year", 2000),
                month=data.get("birthday_month", 1),
                day=data.get("birthday_day", 1),
            )
            if data.get("birthday_year")
            else None
        )

        # 映射其他属性
        self.age = data.get("age", 0)
        self.qid = data.get("qid", "")

        # 映射性别枚举类型
        try:
            self.sex = Sex(data.get("sex", "unknown"))
        except ValueError:
            self.sex = Sex.UNKNOWN

        if self.group_id:
            # 获取群成员信息
            member_info = await self.bot.doAction(
                "get_group_member_info",
                {
                    "group_id": self.group_id,
                    "user_id": self.user_id,
                    "no_cache": force_update,
                },
            )

            member_data = cast(Dict[str, Any], member_info.get("data", {}))

            # 更新群内相关的用户信息
            if member_data:
                # 如果有群名片，则使用群名片覆盖昵称
                if "card" in member_data and member_data["card"]:
                    self.nick = member_data["card"]

                # 存储群内角色信息
                if "role" in member_data:
                    try:
                        self.role = GroupRole(member_data["role"])
                    except ValueError:
                        self.role = GroupRole.UNKNOWN

                # 存储群等级信息
                if "level" in member_data:
                    self.group_level = member_data["level"]

                # 存储禁言状态信息
                if "shut_up_timestamp" in member_data:
                    self.shut_up_timestamp = member_data["shut_up_timestamp"]

                # 存储加入时间信息
                if "join_time" in member_data:
                    self.join_time = datetime.fromtimestamp(member_data["join_time"])

        self._last_update_time = datetime.now()

        return self

    def toDict(self) -> Dict[str, Any]:
        """将用户信息转换为字典

        Returns:
            Dict[str, Any]: 用户信息字典
        """
        result = {
            "qq": self.qq,
            "nick": self.nick,
            "remark": self.remark,
            "country": self.country,
            "city": self.city,
            "reg_time": int(self.reg_time.timestamp()),
            "qid": self.qid,
            "birthday": int(self.birthday.timestamp()) if self.birthday else None,
            "age": self.age,
            "sex": self.sex.value,
            "avatar": self.avatar,
            "user_id": self.user_id,
        }

        # 添加群成员相关信息（如果存在）
        if hasattr(self, "group_id") and self.group_id:
            result["group_id"] = self.group_id
        if hasattr(self, "role") and self.role:
            result["role"] = self.role.value
        if hasattr(self, "group_level") and self.group_level:
            result["group_level"] = self.group_level
        if hasattr(self, "shut_up_timestamp"):
            result["shut_up_timestamp"] = self.shut_up_timestamp
        if hasattr(self, "join_time") and self.join_time:
            result["join_time"] = int(self.join_time.timestamp())

        return result
