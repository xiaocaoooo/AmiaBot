# AmiaBot

AmiaBot 是一个基于 OneBot 协议的 QQ 机器人框架，提供灵活的插件系统、Web 管理界面和丰富的功能支持。

## 目录结构

```
AmiaBot/
├── .gitignore                  # Git 忽略文件配置
├── README.md                   # 项目说明文档（当前文件）
├── amia/                       # 核心功能模块
│   ├── __init__.py             # 核心功能初始化
│   ├── group.py                # 群组相关功能
│   ├── recv_message.py         # 消息接收处理
│   ├── send_message.py         # 消息发送功能
│   └── user.py                 # 用户相关功能
├── build_plugin.ps1            # 插件打包脚本（PowerShell）
├── cache_manager/              # 缓存管理模块
│   └── __init__.py             # 缓存管理初始化
├── config.json                 # 机器人主配置文件
├── config/                     # 配置模块
│   └── __init__.py             # 配置初始化
├── example_plugin/             # 示例插件目录
│   ├── __init__.py             # 插件入口文件
│   └── info.json               # 插件元数据配置
├── fonts/                      # 字体资源目录
│   ├── AaCute.woff             # 可爱风格字体
│   ├── JetBrainsMono-Italic.ttf # JetBrains 等宽斜体
│   ├── JetBrainsMono.ttf       # JetBrains 等宽字体
│   ├── ShangShouFangTangTi.ttf # 上手房唐体
│   └── YurukaStd.ttf           # Yuruka 标准字体
├── logo.svg                    # 项目 Logo
├── main.py                     # 主程序入口
├── openai/                     # OpenAI 集成模块
│   └── __init__.py             # OpenAI 模块初始化
├── package-lock.json           # NPM 依赖锁定文件
├── package.json                # NPM 项目配置
├── plugin_manager/             # 插件管理模块
│   └── __init__.py             # 插件管理器实现
├── requirements.txt            # Python 依赖列表
├── to_image/                   # 转换为图片的功能模块
│   ├── __init__.py             # 图片转换初始化
│   ├── html.py                 # HTML 转图片
│   ├── markdown.py             # Markdown 转图片
│   └── template.html           # HTML 模板
├── tsconfig.json               # TypeScript 配置文件
├── utools/                     # 工具函数集合
│   ├── __init__.py             # 工具初始化
│   ├── command.py              # 命令处理相关工具
│   └── match.py                # 匹配相关工具
└── webui/                      # Web 管理界面模块
    ├── app.py                  # Web 应用入口
    ├── static/                 # 静态资源
    │   ├── css/                # CSS 样式文件
    │   ├── pages/              # 页面资源
    │   └── ts/                 # TypeScript 源码
    └── templates/              # HTML 模板
        ├── login.html          # 登录页面
        └── webui.html          # WebUI 主页面
```

## 环境要求

- Python 3.8 或更高版本
- 依赖项列表（详见 `requirements.txt`）：
  - Werkzeug>=2.0.0
  - Jinja2>=3.0.0
  - MarkupSafe>=2.0.0
  - itsdangerous>=2.0.0
  - click>=8.0.0
  - six>=1.16.0
  - aiohttp
  - psutil
  - pyppeteer

## 安装步骤

1. 克隆项目到本地：
   ```bash
   git clone https://github.com/yourusername/AmiaBot.git
   cd AmiaBot
   ```

2. 安装项目依赖：
   ```bash
   pip install -r requirements.txt
   ```

3. 配置 OneBot 客户端（如 go-cqhttp）并确保其正常运行。

4. 编辑 `config.json` 文件，设置相应的连接参数和配置选项。

## 运行方法

执行主程序文件启动机器人：
```bash
python main.py
```

启动流程：
1. 程序首先检查并创建必要的目录（如 cache、plugins 等）
2. 加载配置文件和环境变量
3. 初始化日志系统
4. 启动 WebUI 管理界面（默认端口为 8080）
5. 加载并初始化插件系统
6. 连接到 OneBot 客户端并开始接收和处理消息

## Web 管理界面

启动机器人后，可以通过浏览器访问 Web 管理界面：
```
http://localhost:8080/
```

WebUI 提供以下功能：
- 查看机器人状态
- 管理插件（启用/禁用/重载）
- 查看日志
- 配置机器人参数

## 插件开发

### 插件结构

一个标准的 AmiaBot 插件应包含以下文件：

```
plugin_name/
├── __init__.py    # 插件主要功能实现
└── info.json      # 插件元数据和配置
```

### 创建 info.json

`info.json` 定义了插件的元数据和触发器配置：

```json
{
  "id": "example_plugin_id",
  "name": "Example Plugin",
  "description": "A test plugin.",
  "version": "1.0.0",
  "author": "Amia",
  "triggers": [
    {
      "id": "echo",
      "type": "text_pattern",
      "func": "echo",
      "params": {
        "pattern": "echo.+"
      }
    }
  ]
}
```

### 创建 __init__.py

`__init__.py` 包含插件的主要功能实现：

```python
from amia.send_message import SendMessage, SendTextMessage
from amia.recv_message import RecvMessage
from plugin_manager import ProjectInterface

# 由 PluginManager 自动注入的项目接口
default_export = {}
project_api: ProjectInterface = None  # type: ignore


async def echo(message: RecvMessage) -> None:
    """处理回显命令的异步函数"""
    # 提取消息文本中 "echo" 后的内容
    text = message.text[4:].strip()
    # 发送回复消息
    await message.reply(SendTextMessage(content=f"Echo: {text}"))
```

### 触发器类型

AmiaBot 支持以下几种触发器类型：

1. **text_pattern**：基于正则表达式匹配消息文本
   ```json
   {
     "type": "text_pattern",
     "params": {
       "pattern": "正则表达式"
     }
   }
   ```

2. **text_command**：基于命令前缀匹配
   ```json
   {
     "type": "text_command",
     "params": {
       "command": "命令名称"
     }
   }
   ```

3. **match_message**：基于消息对象的字段精确匹配
   ```json
   {
     "type": "match_message",
     "params": {
       "field_name": "expected_value"
     }
   }
   ```

### 插件 API

插件可以通过 `project_api` 对象与主程序交互，主要方法包括：

- `send_data_to_project(data_type: str, data: dict)`: 向主程序发送数据

## 消息对象

### 接收消息对象 (RecvMessage)

当接收到消息时，插件会收到一个 `RecvMessage` 对象，包含以下主要属性：

- `message_id`: 消息 ID
- `user_id`: 发送者 QQ 号
- `group_id`: 群组 ID（私聊时为 None）
- `text`: 消息文本内容
- `raw_message`: 原始消息内容
- `sender`: 发送者信息
- `time`: 消息时间戳

主要方法：

- `reply(message: SendMessage)`: 回复消息
- `get_at_users()`: 获取消息中 @ 的用户列表

### 发送消息对象

发送消息时可使用以下类型的对象：

- `SendTextMessage(content: str)`: 文本消息
- `SendImageMessage(image_path: str)`: 图片消息
- `SendAtMessage(user_id: int)`: @ 消息

## 插件打包

完成插件开发后，可以使用项目提供的 `build_plugin.ps1` 脚本将插件打包成 ZIP 文件：

```powershell
powershell -File build_plugin.ps1 -PluginPath .\example_plugin\
```

**注意**：打包脚本中包含硬编码的绝对路径，使用前请根据您的实际环境修改脚本中的路径配置。

## 插件管理

### 启用/禁用插件

AmiaBot 通过修改插件文件名来控制插件的启用状态：
- 启用插件：保持原文件名（如 `plugin_name.py`）
- 禁用插件：添加 `.disabled` 后缀（如 `plugin_name.py.disabled`）

### 热重载插件

可以通过以下方式热重载插件：
1. 通过 WebUI 界面操作
2. 直接修改插件文件，机器人会自动检测并重载

## 日志系统

AmiaBot 的日志保存在 `logs` 目录下，主要包括：
- 运行日志：记录机器人运行状态和错误信息
- 访问日志：记录 API 调用和请求信息
- 使用日志：记录插件使用情况

## 注意事项

1. 确保 OneBot 客户端正确配置并与 AmiaBot 建立连接
2. 首次运行时，程序会自动创建必要的目录结构
3. 修改配置文件后需要重启机器人才能生效
4. 开发插件时，请遵循项目的代码规范和类型注解要求
5. 插件应处理可能出现的异常，避免影响机器人的整体运行

## 免责声明

使用本项目请遵守相关法律法规，不得用于任何非法用途。对于使用本项目所产生的任何后果，作者不承担任何责任。
