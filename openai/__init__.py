import uuid
import aiohttp
import logging
from typing import Dict, Any, List, Optional, Union
from config import ConfigObject

logger = logging.getLogger(__name__)


class ChatMessage:
    """表示OpenAI聊天API中的单条消息"""

    def __init__(self, role: str, content: str):
        """初始化聊天消息

        Args:
            role: 消息的角色，可选值：system, user, assistant
            content: 消息的内容
        """
        if role not in ["system", "user", "assistant"]:
            raise ValueError("消息角色必须是'system', 'user'或'assistant'")

        self.role = role
        self.content = content

    def to_dict(self) -> Dict[str, str]:
        """将消息对象转换为字典格式

        Returns:
            Dict[str, str]: 包含role和content的字典
        """
        return {"role": self.role, "content": self.content}

    def __str__(self) -> str:
        return f"{self.role}: {self.content}"

    def __repr__(self) -> str:
        return f"ChatMessage(role='{self.role}', content='{self.content}')"


class ChatMessageList:
    """表示OpenAI聊天API中的消息列表"""

    def __init__(
        self, messages: Optional[List[Union[Dict[str, str], ChatMessage]]] = None
    ):
        """初始化消息列表

        Args:
            messages: 可选的初始消息列表，可以是字典或ChatMessage对象的列表
        """
        self.messages: List[ChatMessage] = []

        if messages:
            for msg in messages:
                self.add_message(msg)

    def add_message(self, message: Union[Dict[str, str], ChatMessage]):
        """添加一条消息到列表

        Args:
            message: 要添加的消息，可以是字典或ChatMessage对象

        Raises:
            ValueError: 当消息格式不正确时
        """
        if isinstance(message, ChatMessage):
            self.messages.append(message)
        elif isinstance(message, dict):
            if "role" not in message or "content" not in message:
                raise ValueError("消息字典必须包含'role'和'content'字段")
            self.messages.append(ChatMessage(message["role"], message["content"]))
        else:
            raise ValueError("消息必须是ChatMessage对象或包含'role'和'content'的字典")

    def add_system_message(self, content: str):
        """添加一条系统消息

        Args:
            content: 系统消息的内容
        """
        self.add_message(ChatMessage("system", content))

    def add_user_message(self, content: str):
        """添加一条用户消息

        Args:
            content: 用户消息的内容
        """
        self.add_message(ChatMessage("user", content))

    def add_assistant_message(self, content: str):
        """添加一条助手消息

        Args:
            content: 助手消息的内容
        """
        self.add_message(ChatMessage("assistant", content))

    def to_list(self) -> List[Dict[str, str]]:
        """将消息列表转换为API调用所需的格式

        Returns:
            List[Dict[str, str]]: API调用格式的消息列表
        """
        return [msg.to_dict() for msg in self.messages]

    def __len__(self) -> int:
        return len(self.messages)

    def __getitem__(self, index: int) -> ChatMessage:
        return self.messages[index]

    def __iter__(self):
        return iter(self.messages)


class OpenAI:
    """OpenAI API异步调用封装类，仅支持chat/completions接口"""

    _instance: Optional["OpenAI"] = None

    @classmethod
    def get_instance(cls) -> "OpenAI":
        """获取OpenAI实例的单例

        Returns:
            OpenAI: OpenAI实例
        """
        if cls._instance is None:
            raise ValueError("OpenAI实例未初始化。请先调用OpenAI(config)。")
        return cls._instance

    def __init__(self, config: Optional[ConfigObject] = None):
        """初始化OpenAI客户端

        Args:
            config: 包含OpenAI配置的ConfigObject实例，可选参数
        """
        OpenAI._instance = self
        self.api_key = config.api_key if config and hasattr(config, "api_key") else None
        self.api_base = (
            config.api_base
            if config and hasattr(config, "api_base")
            else "https://api.openai.com/v1"
        )
        self.model = (
            config.model if config and hasattr(config, "model") else "gpt-3.5-turbo"
        )
        self.session: Optional[aiohttp.ClientSession] = None

    async def _ensure_session(self, api_key: Optional[str] = None):
        """确保aiohttp会话已创建

        Args:
            api_key: API密钥，如果未提供则使用初始化时的api_key

        Returns:
            aiohttp.ClientSession: 客户端会话实例

        Raises:
            ValueError: 当没有提供API密钥时
        """
        key_to_use = api_key or self.api_key
        if not key_to_use:
            raise ValueError("API密钥未提供，请在初始化或调用时提供")

        if self.session is None or self.session.closed:
            self.session = aiohttp.ClientSession(
                headers={
                    "Authorization": f"Bearer {key_to_use}",
                    "Content-Type": "application/json",
                }
            )
        elif api_key and api_key != self.api_key:
            # 如果提供了不同的API密钥，关闭现有会话并创建新会话
            await self.session.close()
            self.api_key = api_key
            self.session = aiohttp.ClientSession(
                headers={
                    "Authorization": f"Bearer {api_key}",
                    "Content-Type": "application/json",
                }
            )

        return self.session

    async def close(self):
        """关闭aiohttp会话"""
        if self.session and not self.session.closed:
            await self.session.close()
            self.session = None

    async def _request(
        self, method: str, endpoint: str, api_key: Optional[str] = None, **kwargs
    ) -> Dict[str, Any]:
        """发送API请求的内部方法

        Args:
            method: HTTP方法（GET, POST等）
            endpoint: API端点路径
            api_key: API密钥，如果未提供则使用实例中的api_key
            **kwargs: 传递给aiohttp请求的其他参数

        Returns:
            Dict[str, Any]: API响应数据

        Raises:
            Exception: 当API请求失败时
        """
        session = await self._ensure_session(api_key)
        url = f"{self.api_base}{endpoint}"

        try:
            async with session.request(method, url, **kwargs) as response:
                if response.status != 200:
                    error_msg = await response.text()
                    logger.error(f"OpenAI API请求失败: {response.status} - {error_msg}")
                    response.raise_for_status()

                return await response.json()
        except Exception as e:
            logger.error(f"OpenAI API调用错误: {str(e)}")
            raise

    async def chat_completion(
        self,
        messages: Union[List[Dict[str, str]], ChatMessageList, List[ChatMessage]],
        temperature: float = 0.7,
        max_tokens: Optional[int] = None,
        **kwargs,
    ) -> Dict[str, Any]:
        """调用OpenAI聊天完成API

        Args:
            messages: 消息列表，可以是字典列表或ChatMessageList对象
            temperature: 采样温度
            max_tokens: 最大生成token数
            **kwargs: 其他API参数

        Returns:
            Dict[str, Any]: API响应结果

        Raises:
            ValueError: 当没有提供API密钥时
        """
        # 检查API密钥是否存在
        if not self.api_key:
            raise ValueError("API密钥未提供，请在初始化时提供")

        # 处理消息列表格式
        messages_to_use = (
            messages.to_list()
            if isinstance(messages, ChatMessageList)
            else [
                (msg.to_dict() if isinstance(msg, ChatMessage) else msg)
                for msg in messages
            ]
        )

        data = {
            "model": self.model,
            "messages": messages_to_use,
            "temperature": temperature,
        }

        if max_tokens is not None:
            data["max_tokens"] = max_tokens

        # 添加其他可能的参数
        data.update(kwargs)

        # 使用现有的api_base和会话
        return await self._request("POST", "/chat/completions", json=data)

    async def chat(
        self,
        messages: Union[List[Dict[str, str]], ChatMessageList, List[ChatMessage]],
        temperature: float = 0.7,
        max_tokens: Optional[int] = None,
        **kwargs,
    ) -> str:
        """调用OpenAI聊天完成API并返回生成的文本

        Args:
            messages: 消息列表，可以是字典列表或ChatMessageList对象
            temperature: 采样温度
            max_tokens: 最大生成token数
            **kwargs: 其他API参数

        Returns:
            str: 生成的文本

        Raises:
            ValueError: 当没有提供API密钥时
        """
        uid = uuid.uuid4()
        logger.info(f"OpenAI chat completion request [{uid}]: {messages}")
        response = await self.chat_completion(
            messages, temperature=temperature, max_tokens=max_tokens, **kwargs
        )
        resp=response["choices"][0]["message"]["content"]
        logger.info(f"OpenAI chat completion response [{uid}]: {resp}")
        return resp
