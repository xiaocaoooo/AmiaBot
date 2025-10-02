<#
.SYNOPSIS
    构建插件并压缩为plugin文件
.DESCRIPTION
    将plugins_src目录下的所有文件夹视为插件，将单个插件下的所有文件压缩为一个zip文件，
    不保留根文件夹结构，并保存为plugins文件夹下的同名.plugin文件
.EXAMPLE
    .\build_plugin.ps1
#>

# 配置参数
$pluginsSrcDir = "plugins_src"
$pluginsDir = "plugins"
$baseTempDir = Join-Path $env:TEMP "amia_plugin_temp"

# 确保plugins目录存在
if (-not (Test-Path $pluginsDir)) {
    New-Item -Path $pluginsDir -ItemType Directory | Out-Null
}

# 确保基础临时目录存在并清空
if (Test-Path $baseTempDir) {
    Remove-Item -Path $baseTempDir -Recurse -Force
}
New-Item -Path $baseTempDir -ItemType Directory | Out-Null

# 获取plugins_src目录下的所有文件夹
$pluginFolders = Get-ChildItem -Path $pluginsSrcDir -Directory

# 遍历每个插件文件夹
foreach ($pluginFolder in $pluginFolders) {
    $pluginName = $pluginFolder.Name
    $tempDir = Join-Path $baseTempDir $pluginName
    $targetFile = Join-Path $pluginsDir "$pluginName.plugin"
    
    # 确保当前插件的临时目录存在并清空
    if (Test-Path $tempDir) {
        Remove-Item -Path $tempDir -Recurse -Force
    }
    New-Item -Path $tempDir -ItemType Directory | Out-Null
    
    try {
        # 复制插件文件到临时目录（不包括根文件夹）
        Get-ChildItem -Path $pluginFolder.FullName -Recurse | Copy-Item -Destination { 
            Join-Path $tempDir $_.FullName.Substring($pluginFolder.FullName.Length + 1) 
        } -Force
        
        # 切换到临时目录进行压缩，以确保不包含额外的目录层级
        Push-Location -Path $tempDir
        
        # 压缩文件到目标位置
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
        Write-Error "构建插件 $pluginName 时出错: $_"
    } finally {
        # 回到原来的目录
        Pop-Location
    }
}

# 清理基础临时目录
if (Test-Path $baseTempDir) {
    Remove-Item -Path $baseTempDir -Recurse -Force
}

Write-Host "所有插件构建完成！"
