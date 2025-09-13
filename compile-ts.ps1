<#
.SYNOPSIS
    编译TypeScript代码为JavaScript
.DESCRIPTION
    此脚本用于编译AmiaBot WebUI中的TypeScript代码
#>

# 检查Node.js是否已安装
try {
    Get-Command node -ErrorAction Stop
} catch {
    Write-Error "未找到Node.js。请先安装Node.js。"
    Read-Host "按Enter键退出..."
    exit 1
}

# 检查npm是否已安装
try {
    Get-Command npm -ErrorAction Stop
} catch {
    Write-Error "未找到npm。请先安装npm。"
    Read-Host "按Enter键退出..."
    exit 1
}

# 检查是否已安装依赖
if (-not (Test-Path -Path "node_modules")) {
    Write-Host "正在安装依赖..."
    npm install
    if ($LASTEXITCODE -ne 0) {
        Write-Error "安装依赖失败。"
        Read-Host "按Enter键退出..."
        exit 1
    }
}

# 编译TypeScript代码
Write-Host "正在编译TypeScript代码..."
npm run compile-ts
if ($LASTEXITCODE -ne 0) {
    Write-Error "TypeScript编译失败。"
    Read-Host "按Enter键退出..."
    exit 1
}

Write-Host "TypeScript编译成功！编译后的JavaScript文件已生成到webui/static/js目录。"
# Read-Host "按Enter键退出..."
