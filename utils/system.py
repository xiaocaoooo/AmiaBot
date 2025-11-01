import asyncio


async def system(cmd) -> str:
    try:
        # 异步创建子进程执行命令
        # 设置stdout和stderr捕获管道
        process = await asyncio.create_subprocess_shell(
            cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )

        # 异步获取命令输出（stdout和stderr）
        stdout, stderr = await process.communicate()

        # 检查命令执行结果（退出码为0表示成功）
        if process.returncode == 0:
            return stdout.decode('GBK')
        else:
            return f"命令执行失败: {stderr.decode('GBK')}"

    except Exception as e:
        return f"发生异常: {str(e)}"
