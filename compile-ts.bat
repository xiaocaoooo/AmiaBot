@echo off

REM 检查Node.js是否已安装
node -v >nul 2>&1
if %errorlevel% neq 0 (
    echo 错误: 未找到Node.js。请先安装Node.js。
    pause
    exit /b 1
)

REM 检查npm是否已安装
npm -v >nul 2>&1
if %errorlevel% neq 0 (
    echo 错误: 未找到npm。请先安装npm。
    pause
    exit /b 1
)

REM 检查是否已安装依赖
if not exist "node_modules" (
    echo 正在安装依赖...
    npm install
    if %errorlevel% neq 0 (
        echo 安装依赖失败。
        pause
        exit /b 1
    )
)

REM 编译TypeScript代码
npm run compile-ts
if %errorlevel% neq 0 (
    echo TypeScript编译失败。
    pause
    exit /b 1
)

echo TypeScript编译成功！编译后的JavaScript文件已生成到webui/static/js目录。
pause
