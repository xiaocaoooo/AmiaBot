from amia.recv_message import RecvMessage


project_api = None  # 将由 PluginManager 注入

async def echo(message:RecvMessage):
    print(f"Plugin received message: {message.text}")
