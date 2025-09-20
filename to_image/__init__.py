from pathlib import Path
from pyppeteer import launch
import sys
import os


async def toImage(content: str, filename: Path, show_logo: bool = True):
    """将HTML内容渲染为图片

    Args:
        content (str): 要渲染的HTML代码
        filename (Path): 保存的图片路径
        show_logo (bool): 是否显示logo

    Returns:
        None
    """
    # 获取模板文件路径
    template_path = os.path.join(os.path.dirname(__file__), "template.html")

    # 读取模板文件内容
    with open(template_path, "r", encoding="utf-8") as f:
        template_html = f.read()

    # 替换模板中的{content}占位符
    html_content = template_html.replace("{content}", content)

    # 显示logo
    if show_logo:
        with open(
            os.path.join(os.path.dirname(__file__), "..", "logo.svg"),
            "r",
            encoding="utf-8",
        ) as f:
            logo = f.read()
        html_content = html_content.replace("{logo}", f'<div id="logo">{logo}</div>')

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
    browser = await launch(
        headless=True,  # 使用无头模式
        executablePath=chrome_path,
        args=["--no-sandbox", "--disable-setuid-sandbox"],
    )

    # 创建一个新页面
    page = await browser.newPage()

    # 设置页面内容
    await page.setContent(html_content)

    # 等待页面加载完成
    await page.waitForSelector("#app")

    # 截图#app元素并保存
    element = await page.querySelector("#app")
    if element:
        await element.screenshot({"path": str(filename)})

    # 关闭浏览器
    await browser.close()

    return filename
