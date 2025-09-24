from amia.group import Group
from . import Amia
from .user import User
from datetime import datetime
from typing import List, Dict, Any, Optional, ClassVar


class RecvMessage:
    """接收消息的主类，包含消息的基本信息和消息链"""

    message_id: int  # 消息ID
    self_id: int  # 机器人ID
    user_id: int  # 发送者用户ID
    time: datetime  # 消息时间戳
    message_seq: int  # 消息序列号
    real_id: int  # 真实消息ID
    real_seq: str  # 真实消息序列号
    message_type: str  # 消息类型(private/group)
    sender: Optional[User]  # 发送者信息
    raw_message: str  # 原始消息内容
    message: List["RecvBaseMessage"]  # 消息链，解析后的消息列表
    group_id: Optional[int] = None  # 群组ID(群消息时存在)
    group: Optional[Group] = None  # 群组信息(群消息时存在)
    group_name: Optional[str] = None  # 群组名称(群消息时存在)
    raw: Dict[str, Any]  # 原始消息数据
    bot: Amia  # 机器人实例引用

    _instances: ClassVar[Dict[int, "RecvMessage"]] = {}  # 单例缓存

    def __new__(cls, message_id: int, bot: Amia) -> "RecvMessage":
        """创建RecvMessage实例，实现单例模式

        Args:
            message_id: 消息ID
            bot: 机器人实例

        Returns:
            RecvMessage: 消息实例
        """
        if message_id not in cls._instances:
            cls._instances[message_id] = super().__new__(cls)
        return cls._instances[message_id]

    def __init__(self, message_id: int, bot: Amia) -> None:
        """初始化RecvMessage实例

        Args:
            message_id: 消息ID
            bot: 机器人实例
        """
        if not hasattr(self, "_initialized"):
            self.message_id = message_id
            self.bot = bot
            self.self_id = 0
            self.user_id = 0
            self.time = datetime.fromtimestamp(0)
            self.message_seq = 0
            self.real_id = 0
            self.real_seq = ""
            self.message_type = ""
            self.sender = None
            self.raw_message = ""
            self.message = []
            self._initialized = True
            
    @staticmethod
    def fromDict(data: Dict[str, Any], bot: Amia) -> "RecvMessage":
        """从字典创建RecvMessage实例

        Args:
            data: 包含消息数据的字典
            bot: 机器人实例

        Returns:
            RecvMessage: 消息实例
        """
        msg = RecvMessage(data.get("message_id", 0), bot)
        msg.raw = data

        msg.self_id = data.get("self_id", 0)
        msg.user_id = data.get("user_id", 0)
        msg.time = datetime.fromtimestamp(data.get("time", 0))
        msg.message_seq = data.get("message_seq", 0)
        msg.real_id = data.get("real_id", 0)
        msg.real_seq = data.get("real_seq", "")
        msg.message_type = data.get("message_type", "")

        sender_user_id = data.get("sender", {}).get("user_id")
        msg.sender = User(sender_user_id, group_id=data.get("group_id"), bot=bot) if sender_user_id else None

        msg.raw_message = data.get("raw_message", "")
        msg.group_id = data.get("group_id")
        msg.group_name = data.get("group_name")

        raw_messages = data.get("message", [])
        msg.message = [RecvBaseMessage.fromDict(item) for item in raw_messages]
        
        return msg
            
    @property
    def is_group(self) -> bool:
        """判断消息是否为群消息

        Returns:
            bool: 如果是群消息则返回True，否则返回False
        """
        return self.message_type == "group"
    
    @property
    def is_private(self) -> bool:
        """判断消息是否为私聊消息

        Returns:
            bool: 如果是私聊消息则返回True，否则返回False
        """
        return self.message_type == "private"

    async def get_info(self) -> "RecvMessage":
        """异步获取消息的详细信息

        如果消息信息已初始化则直接返回，否则通过API获取完整消息信息

        Returns:
            RecvMessage: 当前消息实例
        """
        if hasattr(self, "self_id") and self.self_id > 0:
            return self

        info = await self.bot.doAction(
            "get_msg", params={"message_id": self.message_id}
        )

        data = info.get("data", {})

        self.raw = data

        self.self_id = data.get("self_id", 0)
        self.user_id = data.get("user_id", 0)
        self.time = datetime.fromtimestamp(data.get("time", 0))
        self.message_seq = data.get("message_seq", 0)
        self.real_id = data.get("real_id", 0)
        self.real_seq = data.get("real_seq", "")
        self.message_type = data.get("message_type", "")

        sender_user_id = data.get("sender", {}).get("user_id")
        self.sender = User(sender_user_id, group_id=data.get("group_id"), bot=self.bot) if sender_user_id else None

        self.raw_message = data.get("raw_message", "")
        self.group_id = data.get("group_id")
        if self.group_id:
            self.group = Group(self.group_id, bot=self.bot)
        self.group_name = data.get("group_name")

        raw_messages = data.get("message", [])
        self.message = [RecvBaseMessage.fromDict(item) for item in raw_messages]

        return self

    @property
    def text(self) -> str:
        """获取消息的文本内容

        Returns:
            str: 消息的文本内容
        """
        return "".join([msg.data.get("text", "") for msg in self.message]).strip()

    async def delete(self) -> Dict[str, Any]:
        """删除消息"""
        return await self.bot.doAction("delete_msg", params={"message_id": self.message_id})

    def toDict(self) -> Dict[str, Any]:
        """将消息对象转换为字典格式

        Returns:
            Dict[str, Any]: 消息的字典表示
        """
        result = {
            "message_id": self.message_id,
            "self_id": self.self_id,
            "user_id": self.user_id,
            "time": int(self.time.timestamp()),
            "message_seq": self.message_seq,
            "real_id": self.real_id,
            "real_seq": self.real_seq,
            "message_type": self.message_type,
            "raw_message": self.raw_message,
            "message": [msg.toDict() for msg in self.message],
        }

        if self.sender is not None:
            result["sender"] = self.sender.qq

        if self.group_id is not None:
            result["group_id"] = self.group_id
        if self.group_name is not None:
            result["group_name"] = self.group_name

        return result

    async def reply(self, send_message: "SendMessage")->"RecvMessage":  # type: ignore # noqa: F821
        """回复消息"""
        return await send_message.reply(self)


class RecvBaseMessage:
    """消息基类，所有具体消息类型的父类"""

    type: str  # 消息类型
    data: Dict[str, Any]  # 消息原始数据

    def toDict(self) -> Dict[str, Any]:
        """将消息对象转换为字典格式

        Returns:
            Dict[str, Any]: 消息的字典表示
        """
        return {"type": self.type, "data": self.data}

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvBaseMessage":
        """静态工厂方法，根据消息类型创建对应的消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvBaseMessage: 对应类型的消息实例
        """
        type_ = data.get("type")
        if type_ == "text":
            return RecvTextMessage.fromDict(data)
        elif type_ == "image":
            return RecvImageMessage.fromDict(data)
        elif type_ == "at":
            return RecvAtMessage.fromDict(data)
        elif type_ == "reply":
            return RecvReplyMessage.fromDict(data)
        elif type_ == "face":
            return RecvFaceMessage.fromDict(data)
        elif type_ == "record":
            return RecvRecordMessage.fromDict(data)
        elif type_ == "video":
            return RecvVideoMessage.fromDict(data)
        elif type_ == "rps":
            return RecvRpsMessage.fromDict(data)
        elif type_ == "dice":
            return RecvDiceMessage.fromDict(data)
        elif type_ == "share":
            return RecvShareMessage.fromDict(data)
        elif type_ == "music":
            return RecvMusicMessage.fromDict(data)
        elif type_ == "poke":
            return RecvPokeMessage.fromDict(data)
        elif type_ == "json":
            return RecvJsonMessage.fromDict(data)
        elif type_ == "markdown":
            return RecvMarkdownMessage.fromDict(data)
        elif type_ == "contact":
            return RecvContactMessage.fromDict(data)
        elif type_ == "mface":
            return RecvMfaceMessage.fromDict(data)
        elif type_ == "file":
            return RecvFileMessage.fromDict(data)
        elif type_ == "node":
            return RecvNodeMessage.fromDict(data)
        elif type_ == "forward":
            return RecvForwardMessage.fromDict(data)
        elif type_ == "location":
            return RecvLocationMessage.fromDict(data)
        elif type_ == "miniapp":
            return RecvMiniappMessage.fromDict(data)
        elif type_ == "xml":
            return RecvXmlMessage.fromDict(data)

        msg = RecvBaseMessage()
        msg.type = type_  # type: ignore
        msg.data = data.get("data", {})
        return msg


class RecvTextMessage(RecvBaseMessage):
    """文本消息类"""

    type = "text"  # 消息类型固定为text
    text: str  # 文本内容

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvTextMessage":
        """从字典创建文本消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvTextMessage: 文本消息实例
        """
        msg = RecvTextMessage()
        msg.data = data.get("data", {})
        msg.text = msg.data.get("text")  # type: ignore
        return msg


class RecvImageMessage(RecvBaseMessage):
    """图片消息类"""

    type = "image"  # 消息类型固定为image
    url: Optional[str] = None  # 图片URL
    file: Optional[str] = None  # 图片文件ID
    sub_type: Optional[str] = None  # 图片子类型
    summary: Optional[str] = None  # 图片摘要

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvImageMessage":
        """从字典创建图片消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvImageMessage: 图片消息实例
        """
        msg = RecvImageMessage()
        msg.data = data.get("data", {})
        msg.url = msg.data.get("url")
        msg.file = msg.data.get("file")
        msg.sub_type = msg.data.get("sub_type")
        msg.summary = msg.data.get("summary")
        return msg


class RecvAtMessage(RecvBaseMessage):
    """@消息类"""

    type = "at"  # 消息类型固定为at
    qq: str  # 被@的用户QQ号
    name: Optional[str] = None  # 被@的用户昵称

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvAtMessage":
        """从字典创建@消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvAtMessage: @消息实例
        """
        msg = RecvAtMessage()
        msg.data = data.get("data", {})
        msg.qq = str(msg.data.get("qq"))
        msg.name = msg.data.get("name")
        return msg


class RecvReplyMessage(RecvBaseMessage):
    """回复消息类"""

    type = "reply"  # 消息类型固定为reply
    id: str  # 回复的消息ID

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvReplyMessage":
        """从字典创建回复消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvReplyMessage: 回复消息实例
        """
        msg = RecvReplyMessage()
        msg.data = data.get("data", {})
        msg.id = str(msg.data.get("id"))
        return msg


class RecvFaceMessage(RecvBaseMessage):
    """表情消息类"""

    type = "face"  # 消息类型固定为face
    id: str  # 表情ID
    resultId: Optional[str] = None  # 结果ID
    chainCount: Optional[int] = None  # 链计数

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvFaceMessage":
        """从字典创建表情消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvFaceMessage: 表情消息实例
        """
        msg = RecvFaceMessage()
        msg.data = data.get("data", {})
        msg.id = str(msg.data.get("id"))
        msg.resultId = msg.data.get("resultId")
        msg.chainCount = msg.data.get("chainCount")
        return msg


class RecvRecordMessage(RecvBaseMessage):
    """语音消息类"""

    type = "record"  # 消息类型固定为record
    file: str  # 语音文件ID
    url: Optional[str] = None  # 语音文件URL

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvRecordMessage":
        """从字典创建语音消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvRecordMessage: 语音消息实例
        """
        msg = RecvRecordMessage()
        msg.data = data.get("data", {})
        msg.file = msg.data.get("file")  # type: ignore
        msg.url = msg.data.get("url")
        return msg


class RecvVideoMessage(RecvBaseMessage):
    """视频消息类"""

    type = "video"  # 消息类型固定为video
    file: str  # 视频文件ID
    url: Optional[str] = None  # 视频文件URL

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvVideoMessage":
        """从字典创建视频消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvVideoMessage: 视频消息实例
        """
        msg = RecvVideoMessage()
        msg.data = data.get("data", {})
        msg.file = msg.data.get("file")  # type: ignore
        msg.url = msg.data.get("url")
        return msg


class RecvRpsMessage(RecvBaseMessage):
    """猜拳消息类"""

    type = "rps"  # 消息类型固定为rps
    result: Optional[int] = None  # 猜拳结果

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvRpsMessage":
        """从字典创建猜拳消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvRpsMessage: 猜拳消息实例
        """
        msg = RecvRpsMessage()
        msg.data = data.get("data", {})
        msg.result = msg.data.get("result")
        return msg


class RecvDiceMessage(RecvBaseMessage):
    """骰子消息类"""

    type = "dice"  # 消息类型固定为dice
    result: Optional[int] = None  # 骰子点数结果

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvDiceMessage":
        """从字典创建骰子消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvDiceMessage: 骰子消息实例
        """
        msg = RecvDiceMessage()
        msg.data = data.get("data", {})
        msg.result = msg.data.get("result")
        return msg


class RecvShareMessage(RecvBaseMessage):
    """分享消息类"""

    type = "share"  # 消息类型固定为share
    url: str  # 分享链接
    title: str  # 分享标题
    content: Optional[str] = None  # 分享内容
    image: Optional[str] = None  # 分享图片URL

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvShareMessage":
        """从字典创建分享消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvShareMessage: 分享消息实例
        """
        msg = RecvShareMessage()
        msg.data = data.get("data", {})
        msg.url = msg.data.get("url")  # type: ignore
        msg.title = msg.data.get("title")  # type: ignore
        msg.content = msg.data.get("content")
        msg.image = msg.data.get("image")
        return msg


# --- 基于message.ts的消息类型扩展 ---


class RecvMusicMessage(RecvBaseMessage):
    """音乐消息类"""

    type = "music"  # 消息类型固定为music
    id: Optional[str] = None  # 音乐ID(用于ID音乐)
    url: Optional[str] = None  # 音乐URL(用于自定义音乐)
    title: Optional[str] = None  # 音乐标题(用于自定义音乐)
    content: Optional[str] = None  # 音乐内容描述(用于自定义音乐)

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvMusicMessage":
        """从字典创建音乐消息实例

        同时处理ID音乐和自定义音乐两种格式

        Args:
            data: 消息数据字典

        Returns:
            RecvMusicMessage: 音乐消息实例
        """
        msg = RecvMusicMessage()
        msg.data = data.get("data", {})
        # 同时处理ID音乐和自定义音乐
        if "id" in msg.data:
            msg.id = msg.data.get("id")
        else:
            msg.url = msg.data.get("url")
            msg.title = msg.data.get("title")
            msg.content = msg.data.get("content")
        return msg


class RecvPokeMessage(RecvBaseMessage):
    """戳一戳消息类"""

    type = "poke"  # 消息类型固定为poke
    poke_type: Optional[str] = None  # 戳一戳类型
    poke_id: Optional[str] = None  # 戳一戳ID

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvPokeMessage":
        """从字典创建戳一戳消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvPokeMessage: 戳一戳消息实例
        """
        msg = RecvPokeMessage()
        msg.data = data.get("data", {})
        msg.poke_type = msg.data.get("type")
        msg.poke_id = msg.data.get("id")
        return msg


class RecvMfaceMessage(RecvBaseMessage):
    """表情(新版)消息类"""

    type = "mface"  # 消息类型固定为mface
    emoji_package_id: Optional[int] = None  # 表情包ID
    emoji_id: Optional[str] = None  # 表情ID
    key: Optional[str] = None  # 表情key
    summary: Optional[str] = None  # 表情摘要

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvMfaceMessage":
        """从字典创建新版表情消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvMfaceMessage: 新版表情消息实例
        """
        msg = RecvMfaceMessage()
        msg.data = data.get("data", {})
        msg.emoji_package_id = msg.data.get("emoji_package_id")
        msg.emoji_id = msg.data.get("emoji_id")
        msg.key = msg.data.get("key")
        msg.summary = msg.data.get("summary")
        return msg


class RecvJsonMessage(RecvBaseMessage):
    """JSON消息类"""

    type = "json"  # 消息类型固定为json
    content: Optional[str] = None  # JSON内容

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvJsonMessage":
        """从字典创建JSON消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvJsonMessage: JSON消息实例
        """
        msg = RecvJsonMessage()
        msg.data = data.get("data", {})
        # JSON消息中的数据字段名称为'data'
        msg.content = msg.data.get("data")
        return msg


class RecvMarkdownMessage(RecvBaseMessage):
    """Markdown消息类"""

    type = "markdown"  # 消息类型固定为markdown
    content: str  # Markdown内容

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvMarkdownMessage":
        """从字典创建Markdown消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvMarkdownMessage: Markdown消息实例
        """
        msg = RecvMarkdownMessage()
        msg.data = data.get("data", {})
        msg.content = msg.data.get("content")  # type: ignore
        return msg


class RecvContactMessage(RecvBaseMessage):
    """联系人消息类"""

    type = "contact"  # 消息类型固定为contact
    contact_type: str  # 联系人类型
    contact_id: str  # 联系人ID

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvContactMessage":
        """从字典创建联系人消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvContactMessage: 联系人消息实例
        """
        msg = RecvContactMessage()
        msg.data = data.get("data", {})
        msg.contact_type = msg.data.get("type")  # type: ignore
        msg.contact_id = msg.data.get("id")  # type: ignore
        return msg


class RecvFileMessage(RecvBaseMessage):
    """文件消息类"""

    type = "file"  # 消息类型固定为file
    file: str  # 文件ID
    name: Optional[str] = None  # 文件名
    url: Optional[str] = None  # 文件URL
    path: Optional[str] = None  # 文件路径
    thumb: Optional[str] = None  # 文件缩略图

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvFileMessage":
        """从字典创建文件消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvFileMessage: 文件消息实例
        """
        msg = RecvFileMessage()
        msg.data = data.get("data", {})
        msg.file = msg.data.get("file")  # type: ignore
        msg.name = msg.data.get("name")
        msg.url = msg.data.get("url")
        msg.path = msg.data.get("path")
        msg.thumb = msg.data.get("thumb")
        return msg


class RecvNodeMessage(RecvBaseMessage):
    """节点消息类(用于合并转发消息中的单个消息)"""

    type = "node"  # 消息类型固定为node
    id: Optional[str] = None  # 消息ID
    user_id: Optional[str] = None  # 用户ID
    uin: Optional[str] = None  # 用户UIN
    nickname: Optional[str] = None  # 用户昵称
    name: Optional[str] = None  # 用户名称
    content: Any  # 消息内容(可能是字符串或消息列表)
    source: Optional[str] = None  # 消息来源
    news: Optional[List[Dict[str, str]]] = None  # 新闻列表
    summary: Optional[str] = None  # 消息摘要
    prompt: Optional[str] = None  # 消息提示
    time: Optional[str] = None  # 消息时间戳

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvNodeMessage":
        """从字典创建节点消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvNodeMessage: 节点消息实例
        """
        msg = RecvNodeMessage()
        msg.data = data.get("data", {})
        msg.id = msg.data.get("id")
        msg.user_id = (
            str(msg.data.get("user_id"))
            if msg.data.get("user_id") is not None
            else None
        )
        msg.uin = str(msg.data.get("uin")) if msg.data.get("uin") is not None else None
        msg.nickname = msg.data.get("nickname")
        msg.name = msg.data.get("name")

        # 处理content内容，可能是字符串或消息数组
        content = msg.data.get("content")
        if isinstance(content, list):
            msg.content = [RecvBaseMessage.fromDict(item) for item in content]
        else:
            msg.content = content

        msg.source = msg.data.get("source")
        msg.news = msg.data.get("news")
        msg.summary = msg.data.get("summary")
        msg.prompt = msg.data.get("prompt")
        msg.time = msg.data.get("time")
        return msg


class RecvForwardMessage(RecvBaseMessage):
    """合并转发消息类"""

    type = "forward"  # 消息类型固定为forward
    id: str  # 合并转发ID
    content: Optional[List[Dict[str, Any]]] = None  # 合并转发内容

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvForwardMessage":
        """从字典创建合并转发消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvForwardMessage: 合并转发消息实例
        """
        msg = RecvForwardMessage()
        msg.data = data.get("data", {})
        msg.id = msg.data.get("id")  # type: ignore
        msg.content = msg.data.get("content")
        return msg


class RecvLocationMessage(RecvBaseMessage):
    """位置消息类"""

    type = "location"  # 消息类型固定为location
    latitude: Optional[float] = None  # 纬度
    longitude: Optional[float] = None  # 经度
    title: Optional[str] = None  # 位置标题
    content: Optional[str] = None  # 位置描述

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvLocationMessage":
        """从字典创建位置消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvLocationMessage: 位置消息实例
        """
        msg = RecvLocationMessage()
        msg.data = data.get("data", {})
        # 处理经纬度数据，可能需要转换为浮点数
        lat = msg.data.get("latitude")
        lon = msg.data.get("longitude")
        msg.latitude = float(lat) if lat is not None else None
        msg.longitude = float(lon) if lon is not None else None
        msg.title = msg.data.get("title")
        msg.content = msg.data.get("content")
        return msg


class RecvMiniappMessage(RecvBaseMessage):
    """小程序消息类"""

    type = "miniapp"  # 消息类型固定为miniapp
    # 小程序消息实际上是json类型的一种特殊形式
    content: Optional[str] = None  # 小程序内容(JSON格式)

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvMiniappMessage":
        """从字典创建小程序消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvMiniappMessage: 小程序消息实例
        """
        msg = RecvMiniappMessage()
        msg.data = data.get("data", {})
        msg.content = msg.data.get("data")
        return msg


class RecvXmlMessage(RecvBaseMessage):
    """XML消息类"""

    type = "xml"  # 消息类型固定为xml
    content: Optional[str] = None  # XML内容字符串

    @staticmethod
    def fromDict(data: Dict[str, Any]) -> "RecvXmlMessage":
        """从字典创建XML消息实例

        Args:
            data: 消息数据字典

        Returns:
            RecvXmlMessage: XML消息实例
        """
        msg = RecvXmlMessage()
        msg.data = data.get("data", {})
        msg.content = msg.data.get("data")
        return msg
