# AmiaBot

一个功能强大、易于扩展的QQ机器人框架，支持插件系统和Web管理界面。

## 功能特点

- **插件系统**：支持动态加载、卸载、启用、禁用插件
- **Web管理界面**：提供直观的可视化管理界面
- **日志系统**：完整的日志记录和查看功能
- **响应式设计**：适配不同设备屏幕尺寸
- **数据可视化**：通过图表展示系统运行状态

## 项目结构

```
AmiaBot/
├── .gitignore           # Git忽略文件配置
├── main.py              # 主程序入口
├── plugin_manager.py    # 插件管理器
├── requirements.txt     # 项目依赖
├── example_plugin/      # 示例插件
└── webui/               # Web管理界面
    ├── app.py           # Web应用程序
    ├── templates/       # HTML模板
    └── static/          # 静态资源
        ├── css/         # CSS样式
        ├── js/          # JavaScript脚本
        └── images/      # 图片资源
```

## 安装步骤

### 1. 克隆项目

```bash
git clone <项目地址>
cd AmiaBot
```

### 2. 创建虚拟环境

```bash
# Windows
python -m venv .venv
.\.venv\Scripts\activate

# macOS/Linux
python3 -m venv .venv
source .venv/bin/activate
```

### 3. 安装依赖

```bash
pip install -r requirements.txt
```

## 使用方法

### 启动机器人

```bash
python main.py
```

启动后，Web管理界面将在 http://localhost:5000 上运行。

### Web管理界面功能

1. **仪表盘**：查看系统状态、插件统计和活动图表
2. **插件管理**：上传、启用、禁用、重新加载插件
3. **日志查看**：查看系统日志，支持级别过滤和搜索
4. **系统设置**：配置系统参数

### 插件开发

1. 创建插件目录结构
2. 在info.json中定义插件信息
3. 实现插件功能，可通过project_api与主程序交互
4. 将插件打包为.zip文件，并重命名为.plugin扩展名
5. 通过Web界面上传插件

## 示例插件

项目中包含了一个示例插件，位于`example_plugin`目录，可以作为开发新插件的参考。

## 配置说明

### 插件目录

默认插件目录为`./plugins`，可以在Web界面的设置中修改。

### 缓存目录

默认缓存目录为`./cache/plugins`，用于临时存放解压的插件文件。

### 日志级别

支持的日志级别有DEBUG、INFO、WARNING、ERROR、CRITICAL，默认为INFO。

## 注意事项

1. 插件文件必须以`.plugin`为扩展名
2. 插件必须包含`info.json`文件，定义插件ID等信息
3. 上传插件后，系统会自动加载新插件
4. 禁用插件后，需要手动重新加载所有插件才能生效

## License

[MIT](LICENSE)