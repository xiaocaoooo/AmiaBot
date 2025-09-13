from datetime import datetime
from enum import Enum
from amia import Amia


class Sex(Enum):
    UNKNOWN = "unknown"
    FEMALE = "female"
    MALE = "male"


class User:
    qq: int
    nick: str
    remark: str
    country: str
    city: str
    reg_time: datetime
    qid: str
    birthday: datetime
    age: int
    sex: Sex

    @property
    def avatar(self) -> str:
        return f"https://q1.qlogo.cn/g?b=qq&nk={self.qq}&s=0"

    def __init__(self, user_id: int, *, group_id: int | None = None, bot: Amia):
        self.user_id = user_id
        self.bot = bot

    async def get_info(self) -> "User":
        """获取用户信息"""
        info = await self.bot.doAction("get_stranger_info", {"user_id": self.user_id})

        data = info.get("data", {})

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

        return self

    def toDict(self) -> dict:
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
