<#
.SYNOPSIS
    构建插件并压缩为plugin文件
.DESCRIPTION
    将指定插件目录下的所有文件压缩为一个zip文件，不保留根文件夹结构，
    并保存为指定的plugin文件
.EXAMPLE
    .\build_plugin.ps1
#>

# 配置参数
$pluginDir = "d:\code\python\qqbot\AmiaBot\example_plugin"
$targetFile = "d:\code\python\qqbot\AmiaBot\plugins\example_plugin.plugin"
$tempDir = Join-Path $env:TEMP "amia_plugin_temp"

# 确保临时目录存在并清空
if (Test-Path $tempDir) {
    Remove-Item -Path $tempDir -Recurse -Force
}
New-Item -Path $tempDir -ItemType Directory | Out-Null

# 复制插件文件到临时目录（不包括根文件夹）
Get-ChildItem -Path $pluginDir -Recurse | Copy-Item -Destination { 
    Join-Path $tempDir $_.FullName.Substring($pluginDir.Length + 1) 
} -Force

# 切换到临时目录进行压缩，以确保不包含额外的目录层级
Push-Location -Path $tempDir

# 压缩文件到目标位置
try {
    # 创建临时zip文件
    $tempZipFile = "$targetFile.zip"
    
    # 如果临时zip文件已存在，先删除
    if (Test-Path $tempZipFile) {
        Remove-Item -Path $tempZipFile -Force
    }
    
    # 如果目标plugin文件已存在，先删除
    if (Test-Path $targetFile) {
        Remove-Item -Path $targetFile -Force
    }
    
    # 使用PowerShell的压缩功能创建zip文件
    Compress-Archive -Path * -DestinationPath $tempZipFile -Force
    
    # 重命名zip文件为plugin文件
    Rename-Item -Path $tempZipFile -NewName (Split-Path $targetFile -Leaf)
    Write-Host "插件构建成功: $targetFile"
} catch {
    Write-Error "构建插件时出错: $_"
} finally {
    # 回到原来的目录
    Pop-Location
    
    # 清理临时目录
    if (Test-Path $tempDir) {
        Remove-Item -Path $tempDir -Recurse -Force
    }
}