from os import system
from pathlib import Path
from typing import Optional, Sequence, Union


def to_cygpath(path: Path) -> str:
    """
    将 Windows Path 对象转换为 Cygwin 格式的路径字符串。

    例如：
      Path("D:\\data") -> "/cygdrive/d/data"
      Path("C:\\Users\\test") -> "/cygdrive/c/Users/test"

    参数:
      path: Path - Windows 路径对象

    返回:
      str - Cygwin 格式的路径字符串
    """
    if path.drive:  # 有盘符（Windows）
        drive_letter = path.drive.strip(":").lower()
        # 去掉盘符后的路径部分，用 / 拼接
        cyg_path = f"/cygdrive/{drive_letter}" + str(path).split(":", 1)[1].replace(
            "\\", "/"
        )
        return cyg_path
    return str(path).replace("\\", "/")


def build_rsync_command(
    source: Path,
    destination: Path,
    remote_host: Optional[str] = None,
    remote_user: Optional[str] = None,
    port: int = 22,
    is_push: bool = True,
    archive: bool = True,
    verbose: bool = True,
    compress: bool = True,
    delete: bool = False,
    dry_run: bool = False,
    exclude_dirs: Optional[Union[Sequence[str], str]] = None,
    ssh:str="ssh",
) -> str:
    """
    生成 rsync 命令字符串（支持本地和远程同步）。

    参数:
      source: Path - 源路径
      destination: Path - 目标路径
      remote_host: Optional[str] - 远程主机名或IP，如果为 None 则表示本地同步
      remote_user: Optional[str] - 远程用户名，如果为 None 则使用当前用户
      port: int - SSH 端口，默认为 22
      is_push: bool - True 表示本地推送到远程，False 表示从远程拉取到本地
      archive: bool - 是否使用归档模式 (-a)
      verbose: bool - 是否显示详细输出 (-v)
      compress: bool - 是否启用压缩 (-z)
      delete: bool - 是否删除目标端不存在的文件 (--delete)
      dry_run: bool - 是否只模拟运行 (--dry-run)
      exclude_dirs: Optional[Union[Sequence[str], str]] - 要排除的目录列表或单个目录字符串
      ssh: str - ssh 命令，默认为 "ssh"
      
    返回:
      str - 生成的 rsync 命令字符串
    """
    cmd = ["rsync"]

    # 选项
    if archive:
        cmd.append("-a")
    if verbose:
        cmd.append("-v")
    if compress:
        cmd.append("-z")
    if delete:
        cmd.append("--delete")
    if dry_run:
        cmd.append("--dry-run")

    # 排除文件夹
    if exclude_dirs:
        if isinstance(exclude_dirs, str):
            exclude_dirs = [exclude_dirs]
        for dir_path in exclude_dirs:
            cmd.extend(["--exclude", f"'{dir_path}'"])

    # SSH 端口
    if remote_host:
        cmd.extend(["-e", f'"{ssh} -p {port}"'])

    # 转换路径为 Cygwin 格式
    src_cyg = to_cygpath(source) + "/"
    dst_cyg = to_cygpath(destination) + "/"

    # 组装源和目标
    if remote_host:
        remote_part = (
            f"{remote_user}@{remote_host}:" if remote_user else f"{remote_host}:"
        )
        if is_push:
            src = src_cyg
            dst = f"{remote_part}{dst_cyg}"
        else:
            src = f"{remote_part}{src_cyg}"
            dst = dst_cyg
    else:
        src = src_cyg
        dst = dst_cyg

    cmd.append(src)
    cmd.append(dst)

    return " ".join(cmd)


if __name__ == "__main__":
    source_path = Path(__file__).parent
    dest_path = Path("/root/qqbot/AmiaBot")

    rsync_cmd = build_rsync_command(
        source=source_path,
        destination=dest_path,
        remote_host="server",
        remote_user="root",
        port=22,
        is_push=True,
        delete=True,
        ssh="C:\\cygwin64\\bin\\ssh.exe",
        exclude_dirs="""
__pycache__
.stfolder
.git
.mypy_cache
.vscode
.venv
venv
amia_data
AmiaOld
cache
data
docs
example_plugin
logs
node_modules
plugins/**.plugin.disabled
plugins_src
plugins_src_disabled
debug.lyric.json
test.html
test.txt
niconico
youtube
onebot
""".strip().split(
            "\n"
        ),
        # dry_run=True,
    )

    print(rsync_cmd)
    if "y" not in input("Run? (y/n) "):
        exit(0)
    system(rsync_cmd)
