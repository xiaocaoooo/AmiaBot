from .html import htmlToImage
from pathlib import Path
import asyncio


async def markdownToImage(
    md_content: str,
    filename: Path,
    *,
    header: str = "",
    script: str = "",
    show_logo: bool = True,
):
    """将Markdown内容渲染为图片

    Args:
        md_content (str): 要渲染的Markdown文本
        filename (Path): 保存的图片路径
        header (str): 自定义HTML头部内容
        show_logo (bool): 是否显示logo

    Returns:
        Path: 保存的图片路径
    """
    header += """
    <script src="https://cdn.jsdelivr.net/npm/marked@12.0.2/lib/marked.umd.min.js"></script>
    <script src="https://cdn.jsdelivr.net/npm/highlight.js@11.9.0/highlight.min.js"></script>
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/highlight.js@11.9.0/styles/github-dark.min.css">"""
    
    # 确保在 JavaScript 模板字符串中的反引号被正确转义
    escaped_md_content = md_content.replace('`', '\\`')

    # 优化后的script
    script += f"""
    <script>
        document.addEventListener('DOMContentLoaded', function() {{
            const markdownText = `{escaped_md_content}`;
            
            // 直接从 window 对象中获取 marked 并使用它的插件接口
            const marked = window.marked;
            const hljs = window.hljs;
            
            // 使用 marked-highlight 官方推荐的方式，直接通过marked.use()来添加高亮功能
            marked.use({{
                highlight: (code, lang) => {{
                    const language = hljs.getLanguage(lang) ? lang : 'plaintext';
                    return hljs.highlight(code, {{ language }}).value;
                }}
            }});
            
            const htmlContent = marked.parse(markdownText);
            document.getElementById('content').innerHTML = htmlContent;
            
            const codeBlocks = document.querySelectorAll('pre code');
            codeBlocks.forEach(block => {{
                block.classList.add('hljs');
                hljs.highlightElement(block);
            }});
            hljs.highlightAll();
        }});
    </script>"""

    return await htmlToImage(
        "", filename, header=header, script=script, show_logo=show_logo
    )
