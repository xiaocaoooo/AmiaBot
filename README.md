# AmiaBot

<div align="center">
  <img src="logo.svg" width="120" height="120" alt="AmiaBot Logo"/>
  <h1>AmiaBot</h1>
  <p>功能强大、易于扩展的QQ机器人框架</p>
  
  <!-- GitHub徽章 -->
  <img src="https://img.shields.io/badge/python-3.8%2B-blue" alt="Python 3.8+"/>
  <img src="https://img.shields.io/badge/license-MIT-green" alt="MIT License"/>
  <img src="https://img.shields.io/badge/OneBot-v11-orange" alt="OneBot v11"/>
  <img src="https://img.shields.io/badge/web-interface-purple" alt="Web Interface"/>
</div>

## 目录

- [项目概述](#项目概述)
- [核心特性](#核心特性)
- [项目结构](#项目结构)
- [环境要求](#环境要求)
- [快速开始](#快速开始)
  - [前提条件](#前提条件)
  - [安装步骤](#安装步骤)
- [使用指南](#使用指南)
  - [启动机器人](#启动机器人)
  - [Web管理界面](#web管理界面)
  - [基础操作示例](#基础操作示例)
- [插件开发](#插件开发)
  - [插件结构](#插件结构)
  - [插件信息定义](#插件信息定义)
  - [核心功能实现](#核心功能实现)
  - [打包与部署](#打包与部署)
  - [示例插件](#示例插件)
- [配置参考](#配置参考)
  - [配置文件说明](#配置文件说明)
  - [目录结构配置](#目录结构配置)
  - [日志配置](#日志配置)
- [API文档](#api文档)
  - [核心接口](#核心接口)
  - [事件处理](#事件处理)
  - [消息发送](#消息发送)
- [常见问题 (FAQ)](#常见问题-faq)
  - [连接问题](#连接问题)
  - [插件问题](#插件问题)
  - [配置问题](#配置问题)
- [注意事项](#注意事项)
- [开发与贡献](#开发与贡献)
  - [贡献流程](#贡献流程)
  - [开发规范](#开发规范)
  - [分支管理](#分支管理)
- [联系方式](#联系方式)
- [License](#license)
- [免责声明](#免责声明)

## 项目概述

AmiaBot是一款功能强大、易于扩展的QQ机器人框架，以Project Sekai的暁山瑞希为主题，基于Python开发并兼容OneBot v11协议。框架通过标准接口与多种QQ客户端（如NapCat、go-cqhttp等）通信，提供灵活的插件系统、直观的Web管理界面和完整的API支持。

AmiaBot旨在为开发者和用户提供一个低门槛、高扩展性的机器人开发与运行平台，适用于个人学习、社区管理、自动化服务等多种场景。通过插件化架构，用户可以根据自身需求快速定制和扩展机器人功能，无需深入了解复杂的底层实现细节。

## 核心特性

### 插件化架构
- **热插拔插件系统**：支持动态加载、卸载、启用和禁用插件，无需重启机器人
- **完整的插件生命周期管理**：提供插件初始化、激活、运行、停用和卸载的全流程支持
- **标准化的插件接口**：统一的插件开发规范，降低开发门槛
- **依赖管理**：支持插件间依赖关系声明与处理

### 友好的用户体验
- **响应式Web管理界面**：提供直观的操作控制台，支持PC端和移动端访问
- **实时数据监控**：通过Web界面实时展示系统运行状态、插件统计和活动日志
- **可视化配置**：支持通过Web界面进行系统配置和插件设置
- **在线插件管理**：支持上传、启用、禁用和重新加载插件

### 技术优势
- **OneBot v11协议兼容**：与多种QQ客户端（NapCat、go-cqhttp等）无缝对接
- **异步处理架构**：基于Python asyncio实现，确保高性能和并发处理能力
- **完善的日志系统**：支持多级别日志记录（DEBUG、INFO、WARNING、ERROR、CRITICAL），便于问题排查和系统监控
- **可扩展的API接口**：提供丰富的API供插件调用，便于功能扩展

### 安全与稳定性
- **权限管理**：支持Web界面密码保护和访问控制
- **异常处理**：完善的异常捕获和处理机制，确保系统稳定运行
- **数据持久化**：配置和状态数据的安全存储和加载机制
- **插件隔离**：插件间相互隔离，单个插件故障不影响整体系统运行

## 项目结构

```
AmiaBot/
├── .gitignore           # Git忽略文件配置
├── README.md            # 项目说明文档
├── amia/                # 核心功能模块
│   ├── __init__.py      # Amia类实现
│   └── user.py          # 用户相关功能
├── compile-ts.bat       # TypeScript编译脚本(Windows)
├── compile-ts.ps1       # TypeScript编译脚本(PowerShell)
├── config.json          # 全局配置文件
├── config/              # 配置相关模块
│   └── __init__.py      # 配置加载器
├── example_plugin/      # 示例插件
│   ├── __init__.py      # 插件实现
│   └── info.json        # 插件信息
├── logo.svg             # 项目Logo
├── main.py              # 主程序入口
├── plugin_manager/      # 插件管理器
│   └── __init__.py      # 插件管理核心实现
├── requirements.txt     # Python项目依赖
├── to_image/            # 图片转换工具
│   ├── __init__.py      # 图片转换实现
│   └── template.html    # 转换模板
└── webui/               # Web管理界面
    ├── app.py           # Web应用主程序
    ├── static/          # 静态资源
    │   ├── css/         # CSS样式
    │   ├── pages/       # 页面相关资源
    │   └── ts/          # TypeScript源码
    └── templates/       # HTML模板
        ├── login.html   # 登录页面
        └── webui.html   # 主界面
```

## 环境要求

- Python 3.8+ 环境
- 已配置OneBot协议的QQ客户端（如NapCat、go-cqhttp等）
- 推荐使用虚拟环境管理项目依赖

## 快速开始

### 前提条件

- 已安装Python 3.8+环境
- 已配置OneBot协议的QQ客户端（如NapCat、go-cqhttp等）

### 安装步骤

1. **克隆项目**

```bash
git clone <项目地址>
cd AmiaBot
```

2. **创建并激活虚拟环境**

```bash
# Windows环境
python -m venv .venv
.\.venv\Scripts\activate

# macOS/Linux环境
python3 -m venv .venv
source .venv/bin/activate
```

3. **安装项目依赖**

```bash
pip install -r requirements.txt
```

4. **配置OneBot服务连接**

编辑`config.json`文件，设置正确的OneBot服务地址和端口：

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
  }
}
```

## 使用指南

### 启动机器人

确保OneBot服务（如NapCat、go-cqhttp等）已成功启动并正常运行，然后执行以下命令启动AmiaBot：

```bash
python main.py
```

程序启动后，你将看到类似以下输出：

```
[INFO] AmiaBot starting...
[INFO] Connecting to OneBot server at 127.0.0.1:3000
[INFO] Web UI running at http://127.0.0.1:8080
[INFO] Plugin manager initialized
[INFO] System ready
```

### Web管理界面

#### 首次访问
1. 打开浏览器，访问配置的Web界面地址（默认：http://localhost:8080）
2. 使用默认用户名（admin）和密码（admin）登录
3. **重要：首次登录后请立即修改密码以保障安全**
4. 在仪表盘查看系统运行状态和基本统计信息

#### 主要功能模块

| 功能模块 | 描述 | 操作说明 |
|---------|------|---------|
| **仪表盘** | 显示系统运行状态、插件统计和活动日志 | 实时监控机器人运行情况，快速了解系统状态 |
| **插件管理** | 上传、启用、禁用、重新加载插件 | 点击"上传插件"按钮选择.plugin文件，上传后可启用或禁用插件 |
| **日志查看** | 查看系统日志，支持按级别过滤和关键词搜索 | 选择日志级别（INFO/WARNING/ERROR等）或输入关键词进行筛选 |
| **系统设置** | 修改Web界面密码、配置插件目录等参数 | 修改后点击"保存"按钮，部分设置需要重启程序生效 |

![Web界面预览]() <!-- 如有截图可添加 -->

### 基础操作示例

#### 1. 插件管理操作

**上传新插件**：
1. 进入"插件管理"页面
2. 点击"上传插件"按钮
3. 选择已打包好的.plugin文件
4. 上传完成后，系统会自动解压并加载插件
5. 点击插件旁的"启用"按钮激活插件

**禁用插件**：
1. 在"插件管理"页面找到目标插件
2. 点击插件旁的"禁用"按钮
3. 确认禁用操作

**重新加载插件**：
1. 在"插件管理"页面找到目标插件
2. 点击插件旁的"重新加载"按钮
3. 插件将被重新初始化

#### 2. 查看和筛选日志

1. 进入"日志查看"页面
2. 从下拉菜单中选择日志级别（如：ERROR）只查看特定级别的日志
3. 在搜索框中输入关键词（如：plugin）筛选包含特定内容的日志
4. 滚动查看日志内容，最新日志显示在底部

#### 3. 修改系统设置

1. 进入"系统设置"页面
2. 修改Web界面密码：在密码字段输入新密码，点击"保存"按钮
3. 配置插件目录：修改插件目录路径，点击"保存"按钮，重启程序生效
4. 调整日志级别：从下拉菜单选择日志级别，点击"保存"按钮，重启程序生效

## 插件开发

AmiaBot提供了灵活而强大的插件系统，使开发者能够轻松扩展机器人功能。本指南将帮助您快速上手插件开发。

### 插件结构

每个插件必须遵循以下目录结构：

```
your_plugin_id/
├── __init__.py    # 插件主程序实现
├── info.json      # 插件信息配置
└── [其他资源文件]  # 插件所需的其他文件（可选）
```

### 插件信息定义

在`info.json`文件中定义插件的基本信息，这是插件被系统识别和加载的必要条件：

```json
{
  "id": "your_unique_plugin_id",     # 插件唯一ID，建议使用英文小写和下划线
  "name": "插件名称",                 # 插件显示名称
  "version": "1.0.0",                # 插件版本，遵循语义化版本规范
  "description": "插件功能的详细描述",  # 插件功能的简要说明
  "author": "您的名称",                # 插件作者
  "dependencies": [                   # 插件依赖的其他插件ID列表
    "required_plugin_id_1",
    "required_plugin_id_2"
  ]
}
```

### 核心功能实现

在`__init__.py`文件中实现插件的核心逻辑。插件必须包含`init_plugin`函数和`main`函数，`main`函数应返回`init_plugin`函数的执行结果。

```python
import logging
from plugin_manager import ProjectInterface

# 插件初始化函数
def init_plugin():
    """插件初始化函数，在插件加载时被调用
    
    Returns:
        dict: 包含初始化状态和消息的字典
            - status: 初始化状态，success或error
            - message: 初始化消息
    """
    try:
        # 获取项目接口实例，用于访问主程序功能
        project_api = ProjectInterface()
        
        # 注册事件处理器（示例）
        def on_message(event):
            """处理收到的消息"""
            message = event.get('message')
            user_id = event.get('user_id')
            group_id = event.get('group_id')
            
            # 示例：回复特定消息
            if 'hello' in message.lower():
                if group_id:
                    project_api.send_group_message(group_id, "Hello! This is AmiaBot!")
                else:
                    project_api.send_private_message(user_id, "Hello! This is AmiaBot!")
        
        # 注册消息事件处理函数
        project_api.register_event_handler('message', on_message)
        
        logging.info(f"插件 {__name__} 已成功初始化")
        
        return {
            "status": "success",
            "message": "插件初始化成功"
        }
    except Exception as e:
        logging.error(f"插件初始化失败: {str(e)}")
        return {
            "status": "error",
            "message": f"初始化失败: {str(e)}"
        }

# 插件入口函数
def main():
    """插件入口函数，由插件管理器调用
    
    Returns:
        dict: 初始化结果字典
    """
    return init_plugin()

if __name__ == "__main__":
    # 直接运行插件时的测试入口
    main()
```

### 打包与部署

完成插件开发后，按照以下步骤打包和部署插件：

1. **打包插件**：
   - 将插件目录中的所有文件打包为ZIP文件
   - 确保`__init__.py`和`info.json`位于ZIP文件的根目录

2. **重命名文件**：
   - 将打包好的ZIP文件重命名，修改扩展名为`.plugin`
   - 例如：`my_plugin.zip` -> `my_plugin.plugin`

3. **部署插件**：
   - 通过Web管理界面的"插件管理"页面上传插件
   - 上传完成后，系统会自动解压并加载插件
   - 点击插件旁的"启用"按钮激活插件

4. **调试插件**：
   - 在Web管理界面的"日志查看"页面查看插件运行日志
   - 根据日志信息进行调试和优化

### 示例插件

项目中提供了完整的示例插件，位于`example_plugin`目录。该示例展示了插件的基本结构、信息配置和功能实现，包括如何注册事件处理器、调用API接口等。建议在开发新插件前，先仔细阅读和理解示例插件的代码。

示例插件实现了一个简单的消息响应功能，当收到包含特定关键词的消息时，会自动回复预设内容。通过研究示例插件，您可以快速了解AmiaBot插件开发的基本流程和最佳实践。

## 配置参考

AmiaBot使用JSON格式的配置文件进行系统设置，主要配置文件为`config.json`。本章节详细介绍各配置项的功能和使用方法。

### 配置文件说明

`config.json`是AmiaBot的主配置文件，包含以下主要配置项：

```json
{
  "onebot": {
    "host": "127.0.0.1",    // OneBot服务主机地址
    "http_port": 3000,       // OneBot HTTP API端口
    "ws_port": 3001          // OneBot WebSocket端口
  },
  "webui": {
    "host": "127.0.0.1",    // Web管理界面主机地址
    "port": 8080,            // Web管理界面端口
    "password": "admin",    // Web管理界面登录密码
    "debug": false           // 是否启用调试模式
  },
  "plugin": {
    "directory": "./plugins",  // 插件存放目录
    "cache_directory": "./cache/plugins"  // 插件缓存目录
  },
  "logging": {
    "level": "INFO",        // 日志级别：DEBUG, INFO, WARNING, ERROR, CRITICAL
    "file_path": null        // 日志文件路径，为null时仅输出到控制台
  }
}
```

### 配置项详细说明

#### OneBot配置

| 配置项 | 类型 | 默认值 | 说明 |
|-------|------|-------|------|
| `onebot.host` | String | `127.0.0.1` | OneBot服务的主机地址，通常为本地地址或远程服务器IP |
| `onebot.http_port` | Integer | `3000` | OneBot服务的HTTP API端口，需与QQ客户端配置一致 |
| `onebot.ws_port` | Integer | `3001` | OneBot服务的WebSocket端口，需与QQ客户端配置一致 |

#### WebUI配置

| 配置项 | 类型 | 默认值 | 说明 |
|-------|------|-------|------|
| `webui.host` | String | `127.0.0.1` | Web管理界面的监听地址，设置为`0.0.0.0`可允许外部访问 |
| `webui.port` | Integer | `8080` | Web管理界面的监听端口 |
| `webui.password` | String | `admin` | Web管理界面的登录密码，**建议首次登录后立即修改** |
| `webui.debug` | Boolean | `false` | 是否启用Web界面的调试模式，调试模式下会输出更详细的日志 |

#### 插件配置

| 配置项 | 类型 | 默认值 | 说明 |
|-------|------|-------|------|
| `plugin.directory` | String | `./plugins` | 插件存放目录，用于存放上传的.plugin文件 |
| `plugin.cache_directory` | String | `./cache/plugins` | 插件缓存目录，用于临时存放解压的插件文件 |

#### 日志配置

| 配置项 | 类型 | 默认值 | 说明 |
|-------|------|-------|------|
| `logging.level` | String | `INFO` | 日志记录级别，可选值：`DEBUG`, `INFO`, `WARNING`, `ERROR`, `CRITICAL` |
| `logging.file_path` | String/null | `null` | 日志文件保存路径，为`null`时仅输出到控制台，设置路径后会同时输出到文件 |

### 目录结构配置

AmiaBot使用以下目录结构来组织文件和数据：

| 目录/文件 | 路径 | 描述 |
|----------|------|------|
| **根目录** | `./` | 项目主目录，包含主要配置文件和入口脚本 |
| **插件目录** | `./plugins` | 用于存放用户上传的.plugin插件文件 |
| **缓存目录** | `./cache/plugins` | 用于临时存放解压的插件文件，便于系统加载 |
| **核心代码** | `./amia/` | 框架核心功能实现 |
| **插件管理器** | `./plugin_manager/` | 插件管理系统实现 |
| **Web界面** | `./webui/` | Web管理界面的源代码和资源文件 |
| **配置文件** | `./config.json` | 系统主配置文件 |

### 修改配置

配置文件可以通过以下两种方式修改：

1. **直接编辑配置文件**：
   - 使用文本编辑器打开`config.json`文件
   - 修改相应配置项的值
   - 保存文件并重启AmiaBot使配置生效

2. **通过Web管理界面修改**：
   - 登录Web管理界面
   - 进入"系统设置"页面
   - 修改相应配置项
   - 点击"保存"按钮
   - 根据提示重启程序（部分配置需要重启才能生效）

### 配置示例

#### 基础配置示例

适合大多数用户的基础配置：

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
    "password": "your_secure_password",
    "debug": false
  },
  "plugin": {
    "directory": "./plugins",
    "cache_directory": "./cache/plugins"
  },
  "logging": {
    "level": "INFO",
    "file_path": null
  }
}
```

#### 高级配置示例

包含日志文件和外部访问的高级配置：

```json
{
  "onebot": {
    "host": "127.0.0.1",
    "http_port": 3000,
    "ws_port": 3001
  },
  "webui": {
    "host": "0.0.0.0",  // 允许从外部访问Web界面
    "port": 80,
    "password": "your_secure_password",
    "debug": false
  },
  "plugin": {
    "directory": "D:/plugins",  // 使用自定义插件目录
    "cache_directory": "D:/cache/plugins"
  },
  "logging": {
    "level": "WARNING",  // 仅记录警告及以上级别的日志
    "file_path": "logs/amia.log"  // 将日志保存到文件
  }
}
```

## 常见问题 (FAQ)

本节汇总了使用AmiaBot过程中常见的问题及其解决方案，按类别分组以便快速查找。

### 安装与环境问题

#### 依赖安装问题

**问：为什么运行程序时提示找不到依赖？**

答：这通常是由于缺少必要的Python库导致的。解决方案：
1. 确保已安装Python 3.8或更高版本
2. 在项目根目录下执行命令：`pip install -r requirements.txt`
3. 如果安装过程中遇到问题，可以尝试使用管理员权限或虚拟环境
4. 对于特定的依赖项，可以单独安装：`pip install 依赖包名称`

#### OneBot连接问题

**问：为什么连接不到OneBot服务？**

答：连接问题可能有多种原因，建议按以下步骤排查：
1. 确认OneBot服务（如NapCat、go-cqhttp）已成功启动且状态正常
2. 检查`config.json`文件中的`onebot`配置项是否与OneBot服务的实际配置一致
   - 验证`host`地址是否正确（通常为127.0.0.1）
   - 确认`http_port`和`ws_port`端口号与OneBot服务配置的端口相同
3. 检查防火墙设置，确保相关端口已开放
4. 尝试重启OneBot服务和AmiaBot

#### Python版本兼容性

**问：AmiaBot支持哪些版本的Python？**

答：AmiaBot推荐使用Python 3.8及以上版本。为获得最佳体验，建议使用Python 3.9或3.10。旧版本的Python可能存在兼容性问题，不建议使用。

### Web管理界面问题

#### 访问问题

**问：为什么Web管理界面无法访问？**

答：请按以下步骤进行排查：
1. 确认AmiaBot程序已成功启动且没有报错
2. 检查`config.json`文件中的`webui`配置项
   - 验证`host`地址是否为127.0.0.1（本地访问）或0.0.0.0（允许外部访问）
   - 确认`port`端口号是否正确
3. 检查防火墙设置，确保Web界面端口已开放
4. 尝试使用不同的浏览器或清除浏览器缓存
5. 确认您访问的URL格式正确（例如：http://127.0.0.1:8080）

#### 登录问题

**问：Web管理界面登录失败怎么办？**

答：如果遇到登录问题：
1. 确认您输入的密码是否正确
   - 默认密码为"admin"，建议首次登录后立即修改
2. 如果忘记密码，可以直接编辑`config.json`文件，修改`webui.password`配置项
3. 尝试清空浏览器缓存或使用隐私模式登录

### 插件管理问题

#### 插件上传与启用

**问：如何上传和启用插件？**

答：通过Web管理界面上传和启用插件的步骤：
1. 登录Web管理界面
2. 点击左侧菜单中的"插件管理"
3. 在插件管理页面，点击"上传插件"按钮
4. 选择要上传的`.plugin`格式插件文件
5. 上传完成后，插件会显示在列表中
6. 找到新上传的插件，点击其对应的"启用"按钮
7. 插件启用后，其功能将立即生效

#### 插件兼容性问题

**问：为什么插件上传后无法正常工作？**

答：插件无法正常工作可能有以下原因：
1. 插件版本与AmiaBot版本不兼容
2. 插件依赖的库未安装
3. 插件配置不正确
4. 插件代码存在错误

解决方案：
- 检查控制台日志，查找与该插件相关的错误信息
- 确认插件是否与您使用的AmiaBot版本兼容
- 有些复杂插件可能需要额外的配置文件或依赖
- 尝试重新上传插件文件

#### 插件删除

**问：如何删除已安装的插件？**

答：删除插件的方法：
1. 登录Web管理界面
2. 进入"插件管理"页面
3. 找到要删除的插件，先点击"禁用"按钮
4. 禁用后，再点击"删除"按钮确认删除
5. 插件删除后，将无法恢复，请谨慎操作

### 功能与使用问题

#### 消息响应问题

**问：为什么机器人不响应消息？**

答：机器人不响应消息可能是以下原因：
1. OneBot服务未正确连接或QQ账号离线
2. 相关插件未启用
3. 消息触发条件未满足
4. 权限设置问题

排查步骤：
- 检查AmiaBot和OneBot服务的连接状态
- 确认相关功能的插件已启用
- 检查插件的配置是否正确
- 查看控制台日志，查找可能的错误信息

#### 性能问题

**问：为什么AmiaBot运行速度变慢？**

答：可能的原因及解决方法：
1. 安装的插件过多：尝试禁用不必要的插件
2. 系统资源不足：关闭其他占用大量资源的程序
3. 日志级别设置过低：在`config.json`中将`logging.level`设置为`WARNING`或`ERROR`
4. 长时间运行：尝试重启AmiaBot以释放资源

### 开发与调试问题

#### 插件开发入门

**问：如何开始开发自定义插件？**

答：开发自定义插件的入门步骤：
1. 详细阅读本文档中的"插件开发"章节
2. 了解插件的基本结构和所需文件
3. 参考"示例插件"部分的代码
4. 按照规范创建`info.json`和`__init__.py`文件
5. 实现插件的核心功能
6. 使用打包工具生成`.plugin`文件
7. 上传到AmiaBot进行测试

#### 插件调试技巧

**问：如何有效地调试插件？**

答：调试插件的方法：
1. **日志输出法**：在插件代码中添加详细的日志输出
   ```python
   import logging
   logger = logging.getLogger(__name__)
   
   def my_function():
       logger.info("函数执行开始")
       # 函数逻辑
       logger.debug(f"变量值: {variable}")
       logger.info("函数执行结束")
   ```

2. **控制台输出**：查看AmiaBot的控制台输出，寻找插件相关的信息和错误
3. **分段测试**：将插件功能拆分为小模块，逐一测试
4. **异常捕获**：添加异常处理，记录详细的错误信息
   ```python
   try:
       # 可能出错的代码
   except Exception as e:
       logger.error(f"发生错误: {str(e)}")
       # 可选：返回错误信息给用户
   ```

### 更新与维护问题

#### 版本更新

**问：如何更新AmiaBot到最新版本？**

答：更新AmiaBot的步骤：
1. **备份数据**：
   - 备份`config.json`配置文件
   - 备份`plugins`目录下的所有插件
   - 如有重要日志，也应备份

2. **下载新版本**：从官方渠道获取最新版本的AmiaBot

3. **替换文件**：
   - 用新版本的文件替换旧版本的文件
   - 保留备份的配置文件和插件目录

4. **恢复配置**：
   - 将备份的`config.json`放回原位置
   - 确保插件目录的路径与配置一致

5. **重启服务**：启动AmiaBot，检查是否正常运行

6. **验证功能**：测试主要功能和已安装的插件是否正常工作

#### 数据备份

**问：如何备份AmiaBot的配置和数据？**

答：定期备份可以防止数据丢失，建议备份以下内容：
1. `config.json` - 主配置文件
2. `plugins`目录 - 所有已安装的插件
3. 如有自定义日志文件，也应备份
4. 重要的运行记录或统计数据

可以手动复制这些文件到安全位置，或使用脚本自动备份。建议定期进行备份，特别是在更新版本或修改重要配置之前。

## 注意事项

使用AmiaBot时，请务必遵守以下注意事项，以确保安全、稳定和合法的使用体验。

### 法律与道德规范

1. **遵守法律法规**：请确保在使用AmiaBot前已阅读并理解您所在国家和地区的相关法律法规，遵守网络空间的法律要求。
2. **合法使用**：**禁止**将AmiaBot用于任何违法、违规或不良用途，包括但不限于传播非法信息、干扰正常网络秩序、侵犯他人权益等。
3. **尊重隐私**：在开发和使用插件时，请尊重用户隐私，未经授权不得收集、存储或分享他人的个人信息。

### 安全防护

4. **数据保护**：请妥善保管您的配置文件、插件及其他敏感数据，避免泄露个人信息、API密钥或其他重要凭据。
5. **插件安全**：使用第三方插件时，请确保插件来源可靠，**避免安装未知来源或未经验证的插件**，以防恶意代码或安全风险。
6. **密码安全**：首次登录Web管理界面后，请**立即修改默认密码**，并定期更换以保障系统安全。
7. **系统权限**：请避免以管理员/root权限运行AmiaBot，以降低潜在安全风险。

### 使用建议

8. **数据备份**：在更新、修改AmiaBot或更改重要配置前，请务必备份所有关键数据，包括配置文件、插件和日志等。
9. **定期更新**：建议定期更新AmiaBot到最新版本，以获取最新功能、性能改进和安全修复。
10. **资源监控**：请注意监控系统资源使用情况，如发现异常高的CPU或内存占用，应检查是否有插件异常或配置问题。
11. **日志检查**：定期查看系统日志，以便及时发现并解决潜在问题。

### 风险提示

12. **兼容性风险**：OneBot协议版本更新或QQ客户端变更可能导致部分功能不可用，请注意官方公告。
13. **稳定性风险**：过度安装插件或配置不当可能影响系统稳定性，建议按需安装必要的插件。
14. **第三方服务依赖**：使用过程中请确保OneBot服务（如NapCat、go-cqhttp）正常运行，其稳定性可能影响AmiaBot的功能。

### 责任声明

使用AmiaBot即表示您同意自行承担使用风险，项目维护者不对因使用AmiaBot而导致的任何直接或间接损失负责。请始终遵循最佳实践，确保安全、合法地使用本软件。

## 开发与贡献

我们非常欢迎社区成员参与AmiaBot的开发和贡献！以下是参与开发和贡献代码的详细指南。

### 开发环境搭建

要开始为AmiaBot开发，请按照以下步骤搭建开发环境：

1. **克隆仓库**
   ```bash
   git clone https://github.com/xiaocaoooo/AmiaBot.git
   cd AmiaBot
   ```

2. **创建虚拟环境**
   ```bash
   # Windows
   python -m venv venv
   .\venv\Scripts\activate
   
   # Linux/macOS
   python3 -m venv venv
   source venv/bin/activate
   ```

3. **安装依赖**
   ```bash
   pip install -r requirements.txt
   pip install -e .  # 以开发模式安装项目
   ```

4. **配置开发环境**
   - 复制示例配置文件：`cp config.example.json config.json`
   - 根据您的环境修改配置项

5. **启动开发服务**
   ```bash
   python main.py --debug
   ```
   `--debug`参数将启用调试模式，提供更详细的日志输出。

### 代码贡献流程

我们遵循标准的GitHub工作流程来管理代码贡献：

1. **Fork项目仓库**
   - 访问 [AmiaBot GitHub仓库](https://github.com/xiaocaoooo/AmiaBot)
   - 点击右上角的"Fork"按钮，将项目复制到您的GitHub账号

2. **创建分支**
   - 在您的Fork中创建一个新的分支进行开发
   - 分支命名建议使用有意义的名称，例如：`feature/your-feature-name` 或 `fix/your-bugfix-name`
   ```bash
   git checkout -b feature/your-feature-name
   ```

3. **开发与提交**
   - 实现您的功能或修复
   - 遵循项目的编码规范（见下文）
   - 确保您的代码通过现有测试
   - 编写清晰的提交信息，描述您的更改
   ```bash
   git add .
   git commit -m "简明扼要的提交信息"
   ```

4. **推送到远程**
   ```bash
   git push origin feature/your-feature-name
   ```

5. **创建Pull Request**
   - 在GitHub上导航到您的Fork
   - 点击"Compare & pull request"按钮
   - 填写PR描述，包括：
     - 您的更改内容和目的
     - 相关的Issue（如有）
     - 任何需要特别注意的事项
   - 提交PR

6. **代码审查**
   - 项目维护者将审查您的代码
   - 根据反馈进行必要的修改
   - 当审查通过后，您的代码将被合并到主分支

### 编码规范

为了保持代码库的一致性和可维护性，请遵循以下编码规范：

1. **Python代码规范**
   - 遵循[PEP 8](https://peps.python.org/pep-0008/)代码风格指南
   - 使用4个空格进行缩进，不使用Tab
   - 每行不超过100个字符
   - 使用类型注解来提高代码可读性
   - 为公共函数和方法添加文档字符串

2. **命名规范**
   - 类名使用PascalCase：`class PluginManager`
   - 函数和变量使用snake_case：`def load_plugin()`, `plugin_name`
   - 常量使用全大写加下划线：`MAX_PLUGIN_COUNT`
   - 私有成员以下划线开头：`_private_method()`

3. **文档规范**
   - 为所有公共API添加详细的文档字符串
   - 文档字符串应包含：功能描述、参数说明、返回值说明和示例
   - 对复杂的业务逻辑添加必要的注释

4. **测试规范**
   - 为新功能编写单元测试
   - 确保测试覆盖率尽可能高
   - 在提交代码前运行所有测试，确保没有引入新的错误

### 报告问题与提出建议

如果您发现了Bug或有新功能建议，请通过以下方式提交：

1. **GitHub Issues**
   - 访问[Issues页面](https://github.com/xiaocaoooo/AmiaBot/issues)
   - 点击"New issue"按钮
   - 选择合适的模板（Bug报告或功能请求）
   - 填写详细信息，包括：
     - 问题的详细描述
     - 重现步骤
     - 预期行为
     - 实际行为
     - 环境信息
     - 相关截图或日志（如有）

### 贡献类型

除了代码贡献外，我们还欢迎以下类型的贡献：

1. **文档改进**：修正错误、添加示例、改进说明等
2. **测试用例**：编写更多测试用例，提高测试覆盖率
3. **Bug报告**：报告您发现的问题，并提供详细信息
4. **功能建议**：提出您认为有价值的新功能
5. **使用案例分享**：分享您使用AmiaBot的经验和案例
6. **社区支持**：在社区中帮助其他用户解决问题

### 行为准则

参与AmiaBot项目的所有贡献者都应遵守以下行为准则：

- 尊重其他贡献者和用户
- 接受建设性批评
- 专注于项目的最佳利益
- 保持友好和包容的态度
- 避免使用冒犯性语言或行为
- 不参与任何形式的骚扰或歧视

通过参与AmiaBot的开发和贡献，您不仅可以帮助改进项目，还能与社区成员共同成长，提高自己的技术能力。我们期待您的参与！

## API文档

本节提供AmiaBot核心API的详细说明，帮助开发者更好地理解和使用系统功能。API文档按功能模块分类，包括核心接口、事件处理和消息发送等。

### 核心接口

核心接口提供了AmiaBot的基础功能，主要用于插件开发和系统集成。

#### `get_bot_info()`

获取机器人的基本信息。

**参数**: 无

**返回值**: 
```python
{
    "bot_id": str,       # 机器人账号ID
    "nickname": str,     # 机器人昵称
    "version": str,      # AmiaBot版本
    "connected": bool,   # 是否连接到OneBot服务
    "online": bool       # QQ账号是否在线
}
```

#### `get_config(section=None, key=None)`

获取系统配置信息。

**参数**: 
- `section` (str, 可选): 配置部分名称，如"onebot"、"webui"等
- `key` (str, 可选): 配置项名称

**返回值**: 
- 如果`section`和`key`都不指定，返回整个配置字典
- 如果只指定`section`，返回该部分的配置字典
- 如果同时指定`section`和`key`，返回该配置项的值

#### `set_config(section, key, value)`

修改系统配置信息。

**参数**: 
- `section` (str): 配置部分名称
- `key` (str): 配置项名称
- `value` (any): 配置项的值

**返回值**: 
```python
{
    "success": bool,     # 操作是否成功
    "message": str       # 操作结果消息
}
```

#### `get_plugin_list(enabled=None)`

获取已安装的插件列表。

**参数**: 
- `enabled` (bool, 可选): 过滤插件状态，True为仅启用的插件，False为仅禁用的插件，None为所有插件

**返回值**: 
```python
[
    {
        "id": str,          # 插件ID
        "name": str,        # 插件名称
        "version": str,     # 插件版本
        "author": str,      # 插件作者
        "description": str, # 插件描述
        "enabled": bool     # 插件是否启用
    },
    # ... 更多插件
]
```

#### `get_plugin_info(plugin_id)`

获取指定插件的详细信息。

**参数**: 
- `plugin_id` (str): 插件ID

**返回值**: 
```python
{
    "id": str,          # 插件ID
    "name": str,        # 插件名称
    "version": str,     # 插件版本
    "author": str,      # 插件作者
    "description": str, # 插件描述
    "enabled": bool,    # 插件是否启用
    "dependencies": list, # 插件依赖
    "config": dict      # 插件配置
}
```

### 事件处理

事件处理API用于监听和响应QQ消息及系统事件，是插件开发的核心部分。

#### `register_event_handler(event_type, handler_func)`

注册事件处理器，用于响应特定类型的事件。

**参数**: 
- `event_type` (str): 事件类型，如"message"、"notice"、"request"等
- `handler_func` (callable): 事件处理函数

**返回值**: 
```python
{
    "success": bool,     # 注册是否成功
    "handler_id": str    # 处理器ID，用于后续注销
}
```

#### `unregister_event_handler(handler_id)`

注销指定的事件处理器。

**参数**: 
- `handler_id` (str): 处理器ID，由`register_event_handler`返回

**返回值**: 
```python
{
    "success": bool,     # 注销是否成功
    "message": str       # 操作结果消息
}
```

#### 事件类型列表

以下是常用的事件类型：

| 事件类型 | 描述 | 事件数据结构 |
|---------|------|------------|
| `message` | 收到消息事件 | `MessageEvent` |
| `notice` | 系统通知事件 | `NoticeEvent` |
| `request` | 请求事件 | `RequestEvent` |
| `meta_event` | 元事件 | `MetaEvent` |

#### `MessageEvent` 数据结构

```python
{
    "time": int,          # 事件发生时间戳
    "self_id": str,       # 机器人账号ID
    "post_type": str,     # 事件类型，固定为"message"
    "message_type": str,  # 消息类型，如"private"、"group"、"discuss"等
    "sub_type": str,      # 消息子类型
    "message_id": int,    # 消息ID
    "user_id": str,       # 发送者ID
    "group_id": str,      # 群聊ID（如果是群消息）
    "message": list,      # 消息内容（消息段列表）
    "raw_message": str,   # 原始消息内容
    "font": int,          # 字体
    "sender": dict        # 发送者信息
}
```

### 消息发送

消息发送API用于向用户、群聊等发送消息，支持文本、图片、语音等多种消息类型。

#### `send_message(message_type, target_id, message, auto_escape=False)`

发送消息到指定目标。

**参数**: 
- `message_type` (str): 消息类型，如"private"、"group"等
- `target_id` (str): 目标ID，用户ID或群聊ID
- `message` (str/list): 消息内容，可以是字符串或消息段列表
- `auto_escape` (bool, 可选): 是否自动转义特殊字符，默认为False

**返回值**: 
```python
{
    "success": bool,     # 发送是否成功
    "message_id": int,   # 消息ID（如果发送成功）
    "error_code": int,   # 错误码（如果发送失败）
    "error_message": str # 错误消息（如果发送失败）
}
```

#### `send_private_message(user_id, message, auto_escape=False)`

发送私聊消息。

**参数**: 
- `user_id` (str): 用户ID
- `message` (str/list): 消息内容
- `auto_escape` (bool, 可选): 是否自动转义特殊字符

**返回值**: 
同`send_message`

#### `send_group_message(group_id, message, auto_escape=False)`

发送群聊消息。

**参数**: 
- `group_id` (str): 群聊ID
- `message` (str/list): 消息内容
- `auto_escape` (bool, 可选): 是否自动转义特殊字符

**返回值**: 
同`send_message`

#### 消息段格式

AmiaBot支持以下类型的消息段：

##### 文本消息段

```python
{
    "type": "text",
    "data": {
        "text": "文本内容"
    }
}
```

##### 图片消息段

```python
{
    "type": "image",
    "data": {
        "file": "图片路径或URL",
        "type": "group"或"private",  # 图片类型
        "subType": 0,                 # 子类型
        "url": "图片URL"             # 图片URL（可选）
    }
}
```

##### 表情消息段

```python
{
    "type": "face",
    "data": {
        "id": "表情ID"
    }
}
```

### API调用示例

以下是一些API调用的示例代码，帮助开发者快速上手：

#### 发送群消息示例

```python
from amia import AmiaBot

# 初始化AmiaBot实例
bot = AmiaBot.get_instance()

# 发送文本消息
result = bot.send_group_message(
    group_id="123456789",
    message="Hello, World!"
)

if result["success"]:
    print(f"消息发送成功，消息ID: {result['message_id']}")
else:
    print(f"消息发送失败: {result['error_message']}")

# 发送带图片的消息
result = bot.send_group_message(
    group_id="123456789",
    message=[
        {"type": "text", "data": {"text": "这是一张图片："}},
        {"type": "image", "data": {"file": "path/to/image.jpg"}}
    ]
)
```

#### 注册消息处理器示例

```python
from amia import AmiaBot

# 初始化AmiaBot实例
bot = AmiaBot.get_instance()

# 定义消息处理函数
def on_message(event):
    # 获取消息内容
    message_text = event["raw_message"]
    
    # 简单的回声功能
    if message_text.startswith("echo "):
        # 提取要回复的内容
        reply_text = message_text[5:]
        
        # 根据消息类型回复
        if event["message_type"] == "private":
            bot.send_private_message(
                user_id=event["user_id"],
                message=reply_text
            )
        elif event["message_type"] == "group":
            bot.send_group_message(
                group_id=event["group_id"],
                message=reply_text
            )

# 注册消息处理器
result = bot.register_event_handler(
    event_type="message",
    handler_func=on_message
)

if result["success"]:
    print(f"消息处理器注册成功，处理器ID: {result['handler_id']}")
else:
    print("消息处理器注册失败")
```

## 联系方式

如有任何问题或建议，欢迎通过以下方式与我们联系：
- GitHub Issues：直接在项目仓库提交问题或建议
- 电子邮件：904228000@qq.com

## License

[![License](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

本项目采用 MIT 许可证开源，详情请查看 LICENSE 文件。

## 免责声明

本项目仅用于学习和研究目的，请勿用于任何违反法律法规的活动。使用本项目产生的一切后果由使用者自行承担。作者不对使用本项目可能导致的任何问题负责。
