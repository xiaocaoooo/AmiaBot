import logging
from pathlib import Path
import traceback
from pyppeteer import launch
import sys
import os
import base64
import aiohttp
from pyppeteer.browser import Browser
from cache_manager import CacheManager
import re
import asyncio
from utools.tries import tries


MAX_DIMENSION = 16384 * 0.6

# 全局浏览器单例
_browser_instance = None

async def get_browser_instance() -> Browser:
    """获取浏览器实例的单例
    
    Returns:
        Browser: 浏览器实例
    """
    global _browser_instance
    
    # 如果浏览器实例不存在，则创建
    if _browser_instance is None:
        # 根据操作系统设置Chrome可执行文件路径
        if sys.platform == "win32":
            chrome_path = R"C:\Program Files\Google\Chrome\Application\chrome.exe"
        elif sys.platform == "darwin":
            chrome_path = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
        else:
            chrome_path = "/usr/bin/google-chrome"

        # 确保Chrome路径存在
        if not os.path.exists(chrome_path):
            # 尝试使用系统默认的Chrome路径
            chrome_path = None

        # 启动浏览器
        _browser_instance = await launch(
            headless=True,
            executablePath=chrome_path,
            args=["--no-sandbox", "--disable-setuid-sandbox"],
        )
    return _browser_instance

async def close_browser():
    """关闭浏览器实例
    
    当应用程序退出时应该调用此函数以释放资源
    """
    global _browser_instance
    
    if _browser_instance is not None:
        await _browser_instance.close()
        _browser_instance = None

async def download_images_to_base64(html_content: str, cache_manager: 'CacheManager') -> str:
    """下载HTML中的图片并转换为base64格式
    
    Args:
        html_content (str): HTML内容
        cache_manager (CacheManager): 缓存管理器实例
        
    Returns:
        str: 处理后的HTML内容，图片已转换为base64
    """
    # 查找所有img标签的正则表达式
    img_pattern = re.compile(r"""<img[^>]*src=["']([^"'>]*)["'][^>]*>""")
    
    # 创建一个字典来存储所有图片的处理结果
    img_results = {}
    
    # 先收集所有需要处理的图片URL
    img_urls = [match.group(1) for match in img_pattern.finditer(html_content) 
               if not match.group(1).startswith('data:')]
    
    # 处理本地缓存检查和下载
    @tries(5)
    async def process_image(src: str) -> tuple[str, str | None]:
        logging.info(f"处理图片: {src}")
        # 检查图片是否已经在缓存中
        cache_path = cache_manager.get_cache_by_id(src, 'base64')
        if cache_path.exists():
            with open(cache_path, 'r', encoding='utf-8') as f:
                return src, f.read()
        
        # 下载图片或读取本地图片
        image_data = None
        if src.startswith('http://') or src.startswith('https://'):
            # 下载网络图片
            async with aiohttp.ClientSession() as session:
                async with session.get(src, timeout=10) as response:
                    response.raise_for_status()
                    image_data = await response.read()
        else:
            # 读取本地图片
            # 尝试作为绝对路径读取
            if os.path.exists(src):
                with open(src, 'rb') as f:
                    image_data = f.read()
            else:
                # 尝试相对于当前脚本的路径读取
                script_dir = os.path.dirname(__file__)
                local_path = os.path.join(script_dir, src)
                if os.path.exists(local_path):
                    with open(local_path, 'rb') as f:
                        image_data = f.read()
                else:
                    # 无法找到图片
                    logging.warning(f"无法找到图片: {src}")
                    return src, None            
        if image_data:
            # 转换为base64
            base64_data = base64.b64encode(image_data).decode('utf-8')
            
            # 保存到缓存
            with open(cache_path, 'w', encoding='utf-8') as f:
                f.write(base64_data)
            
            return src, base64_data
        else:
            raise Exception(f"下载图片失败: {src}")
    
    # 并发处理所有图片
    tasks = []
    for src in img_urls:
        if src not in img_results:
            tasks.append(process_image(src))

    results = await asyncio.gather(*tasks)
    for src, base64_data in results:
        img_results[src] = base64_data
    
    # 处理找到的所有img标签
    def replace_img_tag(match):
        img_tag = match.group(0)
        src = match.group(1)
        
        # 如果已经是data URL，直接返回
        if src.startswith('data:'):
            return img_tag
            
        # 获取处理结果
        base64_data = img_results.get(src)
        
        if base64_data:
            # 获取图片类型
            img_type = 'image/png'
            if src.lower().endswith('.jpg') or src.lower().endswith('.jpeg'):
                img_type = 'image/jpeg'
            elif src.lower().endswith('.gif'):
                img_type = 'image/gif'
            elif src.lower().endswith('.webp'):
                img_type = 'image/webp'
                
            # 构建data URL
            data_url = f"data:{img_type};base64,{base64_data}"
            
            # 替换src属性
            new_img_tag = img_tag.replace(src, data_url)
            return new_img_tag
            
        return img_tag
        
    # 对HTML内容中的所有img标签进行替换
    result = img_pattern.sub(replace_img_tag, html_content)
    return result

async def htmlToImage(
    content: str,
    filename: Path | None = None,
    *,
    header: str = "",
    script: str = "",
    show_logo: bool = True,
    scale: float = -1,
):
    """将HTML内容渲染为图片

    Args:
        content (str): 要渲染的HTML代码
        filename (Path): 保存的图片路径
        show_logo (bool): 是否显示logo
        scale (float): 缩放比例

    Returns:
        Path: 保存的图片路径
    """
    assert CacheManager.has_instance() or filename is not None, "CacheManager 未初始化，且未提供 filename"
    if CacheManager.has_instance():
        cache_manager = CacheManager.get_instance().get_child_cache("htmlToImage")
        
        # 下载并转换图片为base64
        content = await download_images_to_base64(content, cache_manager)
    
    logger = logging.getLogger(__name__)
    logger.info(f"htmlToImage content: {content if len(content) < 100 else content[:100] + '...'}")
    
    if filename is None:
        filename = cache_manager.get_cache("png") # type: ignore
    filename.parent.mkdir(parents=True, exist_ok=True)
    # 获取模板文件路径
    template_path = os.path.join(os.path.dirname(__file__), "template.html")

    # 读取模板文件内容
    with open(template_path, "r", encoding="utf-8") as f:
        template_html = f.read()

    # 读取字体文件并转换为base64
    ShangShouFangTangTi_path = os.path.join(
        os.path.dirname(__file__), "..", "fonts", "ShangShouFangTangTi.ttf"
    )
    with open(ShangShouFangTangTi_path, "rb") as f:
        ShangShouFangTangTi_data = base64.b64encode(f.read()).decode("utf-8")
    YurukaStd_path = os.path.join(os.path.dirname(__file__), "..", "fonts", "YurukaStd.ttf")
    with open(YurukaStd_path, "rb") as f:
        YurukaStd_data = base64.b64encode(f.read()).decode("utf-8")
    AaCute_path = os.path.join(os.path.dirname(__file__), "..", "fonts", "AaCute.woff")
    with open(AaCute_path, "rb") as f:
        AaCute_data = base64.b64encode(f.read()).decode("utf-8")

    # 创建字体CSS
    font_css = f"""
    <style>
        @font-face {{
            font-family: 'ShangShouFangTangTi';
            src: url('data:font/ttf;base64,{ShangShouFangTangTi_data}') format('truetype');
            font-weight: normal;
            font-style: normal;
        }}
        
        @font-face {{
            font-family: 'YurukaStd';
            src: url('data:font/ttf;base64,{YurukaStd_data}') format('truetype');
            font-weight: normal;
            font-style: normal;
        }}
        
        @font-face {{
            font-family: 'AaCute';
            src: url('data:font/woff;base64,{AaCute_data}') format('woff');
            font-weight: normal;
            font-style: normal;
        }}

        body {{
            font-family: 'AaCute', 'ShangShouFangTangTi', 'YurukaStd', 'Microsoft YaHei UI', sans-serif;
        }}
    </style>
    """

    # 替换模板中的{content}占位符
    html_content = template_html.replace("{content}", content)

    # 替换模板中的{header}占位符，添加字体CSS
    html_content = html_content.replace("{header}", font_css + header)

    # 替换模板中的{script}占位符
    html_content = html_content.replace("{script}", script)

    # 显示logo
    if show_logo:
        with open(
            os.path.join(os.path.dirname(__file__), "..", "logo.svg"),
            "r",
            encoding="utf-8",
        ) as f:
            logo = f.read()
        html_content = html_content.replace("{logo}", f'<div id="logo">{logo}</div>')

    # 获取浏览器实例（使用全局单例）
    browser = await get_browser_instance()

    # 创建一个新页面
    page = await browser.newPage()

    # 设置页面内容
    await page.setContent(html_content)
    
    with open("test.html", "w") as f:
        f.write(html_content)

    # 等待页面加载完成，包括网络资源
    await page.waitForSelector("#app")
    
    # 再等待一小段时间确保所有异步操作完成
    await page.waitFor(500)

    # 截图#app元素并保存
    element = await page.querySelector("#app")
    if element:
        # 获取宽高
        dimensions = await element.boundingBox()
        if dimensions:
            # 当scale为-1时，尝试最大缩放比例
            if scale == -1:
                scale = MAX_DIMENSION / max(dimensions["width"], dimensions["height"])
            else:
                # 检查缩放后的尺寸是否超过最大限制
                if (
                    dimensions["width"] * scale > MAX_DIMENSION
                    or dimensions["height"] * scale > MAX_DIMENSION
                ):
                    scale = MAX_DIMENSION / max(dimensions["width"], dimensions["height"])

            await page.setViewport(
                {
                    "width": int(dimensions["width"] * scale),
                    "height": int(dimensions["height"] * scale),
                    "deviceScaleFactor": scale,
                }
            )

            await page.waitForSelector("#app")

            if element:
                # 截图
                await element.screenshot(
                    {
                        "path": str(filename),
                        "type": "png"
                    }
                )
                
    # await page.waitFor(800000)

    await page.close()

    return filename
