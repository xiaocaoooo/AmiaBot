from amia.recv_message import RecvMessage
from amia.send_message import SendMessage, SendTextMessage
from plugin_manager import ProjectInterface


project_api: ProjectInterface = None  # type: ignore # 将由 PluginManager 注入


async def example(message: RecvMessage):
    await message.reply(
        SendMessage(SendTextMessage("这是一个示例回复"), bot=project_api.bot)
    )
