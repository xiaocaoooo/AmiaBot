from amia.recv_message import RecvMessage
from amia.send_message import SendMessage, SendTextMessage
from plugin_manager import ProjectInterface


project_api: ProjectInterface = None  # type: ignore # 将由 PluginManager 注入


async def echo(message: RecvMessage):
    """回显插件"""
    await message.reply(
        SendMessage(
            SendTextMessage(message.text.split("echo", 1)[1].strip()),
            bot=project_api.bot,
        )
    )
