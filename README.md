# AmiaBot

![Python](https://img.shields.io/badge/Python-3.8+-brightgreen.svg)
![License](https://img.shields.io/badge/License-MIT-blue.svg)

AmiaBot 是一个功能强大、易于扩展的 QQ 机器人框架，基于 OneBot 协议实现，提供灵活的插件系统、直观的 Web 管理界面和丰富的功能支持。它采用异步编程模式，性能优异，适合构建各类 QQ 机器人应用。

## ✨ 主要功能

- **完整的消息处理系统**：支持收发文本、图片、表情、语音、视频等多种消息类型
- **灵活的插件机制**：支持动态加载、卸载插件，便于功能扩展
- **OpenAI 集成**：内置对 OpenAI API 的支持，可以轻松接入 AI 功能
- **Web 管理界面**：提供直观的 Web 界面，方便管理机器人和插件
- **高效的缓存管理**：优化数据访问性能，提升机器人响应速度
- **丰富的工具库**：提供多种实用工具，简化开发流程
- **异步编程模式**：基于 Python 的异步特性，性能优异

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

## 🚀 快速开始

### 安装步骤

1. **克隆项目**
   ```bash
   git clone https://github.com/yourusername/AmiaBot.git
   cd AmiaBot
   ```

2. **安装依赖**
   ```bash
   pip install -r requirements.txt
   ```

3. **配置 OneBot 客户端**
   配置 OneBot 客户端（如 go-cqhttp）并确保其正常运行。

4. **编辑配置文件**
   编辑 `config.json` 文件，设置相应的连接参数和配置选项。

### 配置说明

编辑项目根目录下的 `config.json` 文件，根据实际需求进行配置：

```json
{
    "onebot": {
        "host": "127.0.0.1",
        "http_port": 3000,
        "ws_port": 3001
    },
    "webui": {
        "host": "127.0.0.1",
        "port": 8080,
        "password": "admin"
    },
    "info_cache_time": 60000,
    "openai": {
        "api_key": "sk-",
        "api_base": "http://127.0.0.1:3902/v1",
        "model": "gpt-3.5-turbo"
    },
    "prefixes": [
        "/", ",", "，", ".", "。", ":", "：", ";", "；", 
        "!", "！", "?", "？", "#", "~", "*", "%", "-"
    ]
}
```

**配置项说明：**
- `onebot`：OneBot 协议配置，需与 QQ 客户端配置一致
- `webui`：Web 管理界面配置，包括主机地址、端口和访问密码
- `info_cache_time`：信息缓存时间（毫秒）
- `openai`：OpenAI API 配置，包括 API 密钥、基础 URL 和使用的模型
- `prefixes`：命令前缀列表，用于触发机器人响应

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

## 🧩 插件开发

AmiaBot 提供了灵活的插件系统，使开发者能够轻松扩展机器人功能。

### 插件结构

每个插件需要包含两个必要文件：

```
plugin_name/
├── __init__.py    # 插件的主要实现代码
└── info.json      # 插件的元信息配置
```

### 插件示例

以下是一个简单的回显插件示例：

#### `info.json`（插件配置文件）

```json
{
    "id": "example_plugin_id",
    "name": "Example Plugin",
    "description": "A test plugin.",
    "version": "1.0.0",
    "author": "Amia",
    "triggers": [
        {
            "type": "text_pattern",
            "id": "echo",
            "func": "echo",
            "name": "echo",
            "description": "Echo back the message.",
            "params": {
                "pattern": "echo.+"
            }
        }
    ]
}
```

#### `__init__.py`（插件实现代码）

```python
from amia.recv_message import RecvMessage
from amia.send_message import SendMessage, SendTextMessage
from plugin_manager import ProjectInterface


project_api: ProjectInterface = None  # 由 PluginManager 自动注入


async def echo(message: RecvMessage):
    """回显插件主函数"""
    # 提取用户输入的内容（去掉命令前缀）
    content = message.text.split("echo", 1)[1].strip()
    
    # 回复消息
    await message.reply(
        SendMessage(
            SendTextMessage(content),  # 创建文本消息
            bot=project_api.bot,       # 传入机器人实例
        )
    )
```

### 插件安装与加载

将编写好的插件放入 `plugins` 目录中，机器人启动时会自动扫描并加载所有有效的插件。你也可以通过 Web 管理界面管理插件的启用和禁用状态。

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

## 📚 核心 API 介绍

### 消息处理

#### 接收消息

`RecvMessage` 类提供了丰富的方法来处理接收到的消息：
- `text`: 获取消息的纯文本内容
- `is_group`: 判断是否为群消息
- `is_private`: 判断是否为私聊消息
- `reply()`: 回复消息
- `delete()`: 删除消息
- `get_at_users()`: 获取消息中 @ 的用户列表

#### 发送消息

`SendMessage` 类支持发送多种类型的消息：
- `SendTextMessage`: 文本消息
- `SendImageMessage`: 图片消息
- `SendFaceMessage`: 表情消息
- `SendRecordMessage`: 语音消息
- `SendVideoMessage`: 视频消息
- `SendAtMessage`: @消息
- `SendReplyMessage`: 回复消息

### OpenAI 集成

`OpenAI` 类提供了对 OpenAI API 的封装：
- 支持异步调用
- 提供消息列表管理
- 支持自定义模型和 API 端点

### 插件 API

插件可以通过 `project_api` 对象与主程序交互，主要方法包括：
- `send_data_to_project(data_type: str, data: dict)`: 向主程序发送数据

## 🔧 插件打包与管理

### 打包插件

开发完成的插件可以打包为 `.zip` 文件便于分发：

1. 使用项目提供的 `build_plugin.ps1` 脚本进行打包：
   ```powershell
   powershell -File build_plugin.ps1 -PluginPath .\example_plugin\
   ```
2. **注意**：打包脚本中包含硬编码的绝对路径，使用前请根据您的实际环境修改脚本中的路径配置
3. 确保插件目录结构正确，包含所有必要的 `info.json` 和 `__init__.py` 文件

### 安装插件

有两种安装插件的方式：

1. **Web 界面安装**：通过 Web 管理界面上传插件 `.zip` 文件
2. **手动安装**：将解压后的插件文件夹直接放入 `plugins` 目录

### 插件状态管理

AmiaBot 通过文件名后缀控制插件的启用状态：
- **启用插件**：保持原文件名（如 `plugin_name`）
- **禁用插件**：添加 `.disabled` 后缀（如 `plugin_name.disabled`）

### 热重载插件

可以通过以下方式热重载插件：
1. 通过 WebUI 界面进行操作
2. 直接修改插件文件内容，机器人会自动检测并重载

## 📝 日志系统

AmiaBot 使用 Python 内置的 `logging` 模块记录日志，具有以下特点：

### 日志类型

日志保存在 `logs` 目录下，主要分为三类：
- **运行日志**：记录机器人运行状态和错误信息
- **访问日志**：记录 API 调用和请求信息
- **使用日志**：记录插件使用情况

### 日志级别

支持多种日志级别，从低到高：
- `DEBUG`: 调试信息，用于开发阶段
- `INFO`: 一般信息，记录正常运行状态
- `WARNING`: 警告信息，可能出现问题但不影响运行
- `ERROR`: 错误信息，功能无法正常执行
- `CRITICAL`: 严重错误，需要立即处理

## 🌟 总结

AmiaBot 是一个功能强大、易于扩展的 QQ 机器人框架，它具备：

- 完整的消息处理系统，支持多种消息类型
- 灵活的插件机制，让功能扩展变得简单
- OpenAI 集成，支持 AI 能力增强
- Web 管理界面，提供直观的操作体验
- 高效的缓存管理，提升性能
- 丰富的工具库，简化开发过程
- 异步编程模式，确保响应迅速

通过插件系统，开发者可以轻松为 AmiaBot 添加新功能，打造专属的智能机器人助手！

## 免责声明

使用本项目请遵守相关法律法规，不得用于任何非法用途。对于使用本项目所产生的任何后果，作者不承担任何责任。
