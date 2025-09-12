import asyncio
import json

project_api = None  # 将由 PluginManager 注入

async def run_task(data):
    print(f"Plugin received data: {json.dumps(data)}")
    # 验证 project_api 是否存在
    if project_api:
        # 调用项目接口，参数必须是 JSON 对象
        response = await project_api.send_data_to_project({"status": "plugin_task_complete"})
        print(f"Response from project: {response}")
    else:
        print("Project API not available.")

def another_function():
    # 一个同步函数示例
    return "This is a sync function."
