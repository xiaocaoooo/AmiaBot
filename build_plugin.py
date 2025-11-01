import os
import zipfile
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed
import time

# 配置参数
# 使用 Path 对象进行跨平台路径操作
SCRIPT_ROOT = Path(__file__).resolve().parent
PLUGINS_SRC_DIR = SCRIPT_ROOT / "plugins_src"
PLUGINS_DIR = SCRIPT_ROOT / "plugins"

# 配置线程池大小
# 建议设置为 CPU 核心数的几倍，但不要太大，以免引入过多开销
MAX_WORKERS = min(32, (os.cpu_count() or 1) * 4) 

def build_single_plugin(plugin_folder: Path):
    """
    负责构建单个插件的同步函数。
    这个函数将在线程池中被并发执行。
    """
    plugin_name = plugin_folder.name
    target_file = PLUGINS_DIR / f"{plugin_name}.plugin"
    
    # 确保 plugins 目录存在 (在主函数中已经处理，这里作为保险)
    PLUGINS_DIR.mkdir(exist_ok=True)
    
    print(f"[Thread-{os.getpid()}] 开始处理插件: {plugin_name}")

    try:
        # 1. 确保目标文件不存在
        if target_file.exists():
            os.remove(target_file)
        
        # 2. 创建 zip 文件 (重命名为 .plugin)
        with zipfile.ZipFile(target_file, 'w', zipfile.ZIP_DEFLATED) as zf:
            # 遍历插件目录下的所有文件和文件夹
            for root, _, files in os.walk(plugin_folder):
                root_path = Path(root)
                relative_path = root_path.relative_to(plugin_folder)

                # 添加文件
                for file in files:
                    full_file_path = root_path / file
                    
                    # 构造 zip 归档中的文件名，并使用 Path.as_posix() 确保路径分隔符为 '/'
                    if relative_path == Path('.'):
                        zip_path = file
                    else:
                        zip_path = (relative_path / file).as_posix()
                    
                    # 将文件添加到 zip 归档 (这是 I/O 密集操作)
                    zf.write(full_file_path, zip_path)
                    
        print(f"✅ 插件构建成功: {plugin_name} -> {target_file}")
        return f"插件 {plugin_name} 构建成功"

    except Exception as e:
        print(f"❌ 构建插件 {plugin_name} 时出错: {e}")
        # 出现错误时清理可能创建的失败文件
        if target_file.exists():
            os.remove(target_file)
        return f"插件 {plugin_name} 构建失败: {e}"


def build_plugins_concurrent():
    """
    使用 ThreadPoolExecutor 并发构建所有插件。
    """
    start_time = time.time()
    print(f"Plugins Source Directory: {PLUGINS_SRC_DIR}")
    print(f"Plugins Target Directory: {PLUGINS_DIR}")
    print(f"Max Workers (Threads): {MAX_WORKERS}")

    # 确保 plugins 目录存在
    PLUGINS_DIR.mkdir(exist_ok=True)

    # 获取所有插件文件夹
    plugin_folders = [p for p in PLUGINS_SRC_DIR.iterdir() if p.is_dir()]
    
    if not plugin_folders:
        print("未找到任何插件目录 (plugins_src 下没有子文件夹)。")
        return

    # 使用 ThreadPoolExecutor 并发执行插件构建任务
    with ThreadPoolExecutor(max_workers=MAX_WORKERS) as executor:
        # 提交所有任务到线程池
        future_to_plugin = {
            executor.submit(build_single_plugin, folder): folder.name 
            for folder in plugin_folders
        }
        
        print(f"\n--- 已提交 {len(plugin_folders)} 个插件构建任务 ---")

        # 迭代已完成的任务并打印结果
        for future in as_completed(future_to_plugin):
            plugin_name = future_to_plugin[future]
            try:
                result = future.result()
                print(f"[完成] {result}")
            except Exception as exc:
                print(f"[错误] 插件 {plugin_name} 生成了一个异常: {exc}")

    end_time = time.time()
    elapsed_time = end_time - start_time
    
    print("\n==============================")
    print(f"所有插件构建完成！ 总耗时: {elapsed_time:.2f} 秒")
    print("==============================")

if __name__ == "__main__":
    build_plugins_concurrent()
