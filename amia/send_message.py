from pathlib import Path
from typing import Any, Dict, List, Union
from . import Amia
from .recv_message import RecvMessage
import os
from urllib.parse import urlparse

message_history:Dict[int, List[int]]={} # 被回复ID -> 回复ID列表

class SendMessage:
    def __init__(
        self,
        messages: List["SendBaseMessage"] | "SendBaseMessage",
        bot: Amia,
        *,
        user_id: str | None = None,
        group_id: str | None = None,
    ) -> None:
        self.messages = messages if isinstance(messages, list) else [messages]
        self.user_id = user_id
        self.group_id = group_id
        self.bot = bot

    async def send(
        self,
        *,
        recv_message: RecvMessage | None = None,
        user_id: str | None = None,
        group_id: str | None = None,
    ):
        """发送消息"""
        assert (
            self.user_id or self.group_id or recv_message or user_id or group_id
        ), "user_id or group_id or recv_message is required"
        is_private = (
            self.user_id or user_id or (recv_message and recv_message.is_private)
        )
        is_group = self.group_id or group_id or (recv_message and recv_message.is_group)
        if isinstance(self.messages[0], SendForwardMessage):
            data = await self.bot.doAction(
                "send_forward_msg",
                params={
                    "user_id" if is_private else "group_id": self.user_id or user_id or recv_message.user_id if is_private else self.group_id or group_id or recv_message.group_id,  # type: ignore
                }
                | self.messages[0].data,
            )
            if recv_message:
                message_history.setdefault(recv_message.message_id, []).append(data["data"]["message_id"])
            return RecvMessage(data["data"]["message_id"], self.bot)
        if is_private:
            data = await self.bot.doAction("send_private_msg", params={"user_id": self.user_id or user_id or recv_message.user_id, "message": [m.toDict() for m in self.messages]})  # type: ignore
            if recv_message:
                message_history.setdefault(recv_message.message_id, []).append(data["data"]["message_id"])
            return RecvMessage(data["data"]["message_id"], self.bot)
        elif is_group:
            data = await self.bot.doAction("send_group_msg", params={"group_id": self.group_id or group_id or recv_message.group_id, "message": [m.toDict() for m in self.messages]})  # type: ignore
            if recv_message:
                message_history.setdefault(recv_message.message_id, []).append(data["data"]["message_id"])
            return RecvMessage(data["data"]["message_id"], self.bot)

    async def reply(self, recv_message: RecvMessage):
        """回复消息"""
        if isinstance(self.messages[0], SendForwardMessage):
            return await self.send(recv_message=recv_message)
        new_messages = [
            SendReplyMessage(message_id=recv_message.message_id)
        ] + self.messages
        if recv_message.group_id:
            data = await self.bot.doAction(
                "send_group_msg",
                params={
                    "group_id": recv_message.group_id,
                    "message": [m.toDict() for m in new_messages],
                },
            )
            if recv_message.message_id:
                message_history.setdefault(recv_message.message_id, []).append(data["data"]["message_id"])
            return RecvMessage(data["data"]["message_id"], self.bot)
        elif recv_message.user_id:
            data = await self.bot.doAction(
                "send_private_msg",
                params={
                    "user_id": recv_message.user_id,
                    "message": [m.toDict() for m in new_messages],
                },
            )
            if recv_message.message_id:
                message_history.setdefault(recv_message.message_id, []).append(data["data"]["message_id"])
            return RecvMessage(data["data"]["message_id"], self.bot)


class SendBaseMessage:
    def __init__(self, type: str, data: Dict[str, Any] | None = None) -> None:
        self.type = type
        self.data = data or {}
        
    @classmethod
    def fromDict(cls, data: Dict[str, Any]) -> "SendBaseMessage":
        if data["type"] == "text":
            return SendTextMessage(data["data"]["text"])
        elif data["type"] == "image":
            return SendImageMessage(data["data"]["file"])
        elif data["type"] == "face":
            return SendFaceMessage(data["data"]["id"])
        elif data["type"] == "record":
            return SendRecordMessage(data["data"]["file"])
        elif data["type"] == "video":
            return SendVideoMessage(data["data"]["file"])
        else:
            return cls(data["type"], data["data"])

    def toDict(self) -> Dict[str, Any]:
        return {"type": self.type, "data": self.data}


class SendTextMessage(SendBaseMessage):
    def __init__(self, text: str) -> None:
        super().__init__("text", data={"text": text})


class SendImageMessage(SendBaseMessage):
    def __init__(self, image: str | Path) -> None:
        if isinstance(image, Path):
            image = str(image)
        if urlparse(image).scheme in ("http", "https", "file"):
            img_path = image
        else:
            abs_path = os.path.abspath(image)
            img_path = f'file://{abs_path.replace(os.sep, "/")}'
        super().__init__("image", data={"file": img_path})


class SendFaceMessage(SendBaseMessage):
    def __init__(self, face_id: int) -> None:
        """发送系统表情

        Args:
            face_id: 表情ID，参考 https://bot.q.qq.com/wiki/develop/api-v2/openapi/emoji/model.html#EmojiType
        """
        super().__init__("face", data={"id": face_id})


class SendRecordMessage(SendBaseMessage):
    def __init__(self, file_path: str | Path) -> None:
        """发送语音消息

        Args:
            file_path: 语音文件路径，可以是本地路径、网络路径
        """
        if isinstance(file_path, Path):
            file_path = str(file_path)
        if urlparse(file_path).scheme in ("http", "https", "file"):
            audio_path = file_path
        else:
            abs_path = os.path.abspath(file_path)
            audio_path = f'file://{abs_path.replace(os.sep, "/")}'
        super().__init__("record", data={"file": audio_path})


class SendVideoMessage(SendBaseMessage):
    def __init__(self, file_path: str | Path) -> None:
        """发送视频消息

        Args:
            file_path: 视频文件路径，可以是本地路径、网络路径
        """
        if isinstance(file_path, Path):
            file_path = str(file_path)
        if urlparse(file_path).scheme in ("http", "https", "file"):
            video_path = file_path
        else:
            abs_path = os.path.abspath(file_path)
            video_path = f'file://{abs_path.replace(os.sep, "/")}'
        super().__init__("video", data={"file": video_path})


class SendAtMessage(SendBaseMessage):
    def __init__(self, qq: Union[str, int], name: str | None = None) -> None:
        """发送艾特消息

        Args:
            qq: 用户ID，可以是字符串或数字，"all"表示艾特全体成员
            name: 可选，艾特显示的名称
        """
        data = {"qq": qq}
        if name:
            data["name"] = name
        super().__init__("at", data=data)


class SendReplyMessage(SendBaseMessage):
    def __init__(self, message_id: int) -> None:
        """发送回复消息

        Args:
            message_id: 要回复的消息ID
        """
        super().__init__("reply", data={"id": message_id})


class SendMusicMessage(SendBaseMessage):
    def __init__(self, type: str, id: str) -> None:
        """发送音乐卡片消息

        Args:
            type: 音乐平台类型，如"163"（网易云音乐）、"qq"（QQ音乐）等
            id: 音乐ID
        """
        super().__init__("music", data={"type": type, "id": id})


class SendDiceMessage(SendBaseMessage):
    def __init__(self, result: int | None = None) -> None:
        """发送骰子表情

        Args:
            result: 骰子点数（1-6），根据文档说明，该参数暂不可用
        """
        data = {}
        if result is not None:
            data["result"] = result
        super().__init__("dice", data=data)


class SendFileMessage(SendBaseMessage):
    def __init__(self, file_path: str | Path, filename: str | None = None) -> None:
        """发送文件消息

        Args:
            file_path: 文件路径，可以是本地路径、网络路径或base64编码
            file_name: 可选，显示的文件名
        """
        if isinstance(file_path, Path):
            file_path = str(file_path)
        if urlparse(file_path).scheme not in (
            "http",
            "https",
            "file",
        ) and not file_path.startswith("data:"):
            abs_path = os.path.abspath(file_path)
            file_path = f'file://{abs_path.replace(os.sep, "/")}'

        data = {"file": file_path}
        if filename:
            data["name"] = filename
        elif urlparse(file_path).scheme not in (
            "http",
            "https",
            "file",
        ) and not file_path.startswith("data:"):
            # 如果没有指定文件名且是本地文件路径，从路径中提取文件名
            data["name"] = os.path.basename(file_path)

        super().__init__("file", data=data)


class SendPokeMessage(SendBaseMessage):
    def __init__(self, user_id: Union[str, int]) -> None:
        """发送戳一戳消息

        Args:
            user_id: 要戳的用户ID
        """
        # 戳一戳是特殊的API调用，不使用常规的message数组格式
        # 这里定义这个类是为了保持一致性
        super().__init__("poke", data={"user_id": user_id})


class SendJsonMessage(SendBaseMessage):
    def __init__(self, json_data: str) -> None:
        """发送JSON消息

        Args:
            json_data: JSON格式的消息数据
        """
        super().__init__("json", data={"json": json_data})


class SendRpsMessage(SendBaseMessage):
    def __init__(self, result: int | None = None) -> None:
        """发送猜拳表情

        Args:
            result: 猜拳结果（1-3），1-石头，2-剪刀，3-布，根据文档说明，该参数暂不可用
        """
        data = {}
        if result is not None:
            data["result"] = result
        super().__init__("rps", data=data)


class ForwardMessageItem:
    def __init__(
        self, user_id: str, nickname: str, content: List[SendBaseMessage]
    ) -> None:
        """合并转发消息中的单条消息项

        Args:
            user_id: 发送者ID
            nickname: 发送者昵称
            content: 消息内容，必须是SendBaseMessage对象的列表
        """
        self.user_id = user_id
        self.nickname = nickname
        self.content = content

    def toDict(self) -> Dict[str, Any]:
        """转换为字典格式

        Returns:
            字典格式的消息项
        """
        # 将SendBaseMessage对象列表转换为字典格式列表
        formatted_content = [msg.toDict() for msg in self.content]
        return {
            "type": "node",
            "data": {
                "user_id": self.user_id,
                "nickname": self.nickname,
                "content": formatted_content,
            },
        }


class SendForwardMessage(SendBaseMessage):
    message: List[ForwardMessageItem]

    def __init__(
        self,
        messages: List[ForwardMessageItem],
        *,
        news: List[str] | None = None,
        title: str | None = None,
    ) -> None:
        """发送合并转发消息

        Args:
            messages: 要转发的消息列表，每项必须是ForwardMessageItem对象
        """
        self.message = messages
        self.news = news
        self.title = title
        # 确保所有消息项都转换为字典格式
        self.formatted_messages: List[Dict[str, Any]] = [
            msg.toDict() for msg in messages
        ]

        data: Dict[str, Any] = {"messages": self.formatted_messages}
        if news:
            data["news"] = [{"text": news_item} for news_item in news]
        if title:
            data["prompt"] = title
            data["summary"] = title
            data["source"] = title

        super().__init__("forward", data=data)


class SendCustomMusicMessage(SendBaseMessage):
    def __init__(self, title: str, desc: str, url: str, audio: str, cover: str) -> None:
        """发送自定义音乐卡片消息

        Args:
            title: 音乐标题
            desc: 音乐描述
            url: 音乐网页链接
            audio: 音频文件链接
            cover: 封面图片链接
        """
        super().__init__(
            "custom_music",
            data={
                "title": title,
                "desc": desc,
                "url": url,
                "audio": audio,
                "cover": cover,
            },
        )


# 辅助函数：创建消息链
def create_message_chain(*messages: SendBaseMessage) -> List[Dict[str, Any]]:
    """创建消息链，将多个消息对象转换为API调用所需的格式

    Args:
        *messages: 多个消息对象

    Returns:
        消息链列表，可直接用于API调用
    """
    return [msg.toDict() for msg in messages]


# 示例用法
# if __name__ == "__main__":
#     # 创建一个艾特+文本的消息链
#     messages = create_message_chain(
#         SendAtMessage("all"),
#         SendTextMessage("大家好！这是一条测试消息"),
#         SendImageMessage("path/to/image.jpg"),
#         SendFaceMessage(37)
#     )
#
#     # 发送群消息
#     # send_message = SendMessage(messages, bot, group_id="123456789")
#     # await bot.send_group_message(send_message)
#
#     # 创建私聊消息
#     # send_message = SendMessage(messages, bot, user_id="987654321")
#     # await bot.send_private_message(send_message)
#
#     # 创建合并转发消息
#     # forward_messages = [
#     #     ForwardMessageItem(
#     #         user_id="123456",
#     #         nickname="用户A",
#     #         content=[SendTextMessage("这是一条被转发的消息")]
#     #     ),
#     #     ForwardMessageItem(
#     #         user_id="654321",
#     #         nickname="用户B",
#     #         content=[
#     #             SendTextMessage("这是一条包含图片的转发消息"),
#     #             SendImageMessage("path/to/image.jpg")
#     #         ]
#     #     )
#     # ]
#     #
#     # forward_message = SendForwardMessage(forward_messages)
#     # send_message = SendMessage([forward_message], bot, group_id="123456789")
#     # await bot.send_group_message(send_message)
