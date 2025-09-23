from enum import Enum
from datetime import datetime, timedelta
from typing import Any, Dict, Optional, cast
from . import Amia


class Sex(Enum):
    """用户性别枚举"""

    UNKNOWN = "unknown"  # 未知性别
    FEMALE = "female"  # 女性
    MALE = "male"  # 男性


class User:
    """用户类，包含用户的基本信息和操作方法"""

    qq: int  # QQ号码
    nick: str  # 昵称
    remark: str  # 备注
    country: str  # 国家
    city: str  # 城市
    reg_time: datetime  # 注册时间
    qid: str  # QID
    birthday: datetime  # 生日
    age: int  # 年龄
    sex: Sex  # 性别
    _last_update_time: datetime  # 最后更新时间

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
        self.birthday = datetime(
            year=data.get("birthday_year", 2000),
            month=data.get("birthday_month", 1),
            day=data.get("birthday_day", 1),
        )

        # 映射其他属性
        self.age = data.get("age", 0)
        self.qid = data.get("qid", "")

        # 映射性别枚举类型
        try:
            self.sex = Sex(data.get("sex", "unknown"))
        except ValueError:
            self.sex = Sex.UNKNOWN

        self._last_update_time = datetime.now()

        return self

    def toDict(self) -> Dict[str, Any]:
        """将用户信息转换为字典

        Returns:
            Dict[str, Any]: 用户信息字典
        """
        return {
            "qq": self.qq,
            "nick": self.nick,
            "remark": self.remark,
            "country": self.country,
            "city": self.city,
            "reg_time": int(self.reg_time.timestamp()),
            "qid": self.qid,
            "birthday": int(self.birthday.timestamp()),
            "age": self.age,
            "sex": self.sex.value,
            "avatar": self.avatar,
        }
