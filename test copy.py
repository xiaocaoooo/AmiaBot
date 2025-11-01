from to_image.html import htmlToImage
import asyncio
import logging
from pathlib import Path
from to_image.html import close_browser

# 配置日志
logging.basicConfig(
    level=logging.INFO, format="%(asctime)s - %(name)s - %(levelname)s - %(message)s"
)


async def test_answer_display():
    """测试将答案文本渲染为美观的图片"""
    logging.info("测试答案文本渲染")

    # 准备答案内容
    answers = [
        "A post office.",
        "By bus.",
        "Have a stomachache.",
        "By taxi.",
        "About 30 kilometers.",
        "15-20 dollars.",
        "Bus Number One.",
        "Eight hours.",
        "Sixty.",
        "Enjoy his family time with his grandchildren.",
    ]

    questions = ["What do you think of Guangzhou?", "What's your dad's job?"]

    # Amy的作文
    composition = "Amy Green is a middle school student. She was born in London but now she lives in Guangzhou. Six years ago, she moved here to stay with her parents because they both work here. Her parents love the city very much. And she likes it, too. It's really a beautiful city. People here are very nice and hard-working. She wants to be a doctor in the future. She will work in this city. To make her dream come true, she will study hard. She likes studying and speaking Chinese, though it is very difficult for her. She often talks with her classmates in Chinese. She also likes Cantonese opera. It's so attractive. She loves it."

    # 创建HTML内容
    content = f"""
<div class="test-container">
    <div class="section-title">
        <h2>英语测试答案</h2>
    </div>
    
    <div class="answers-section">
        <h3 class="section-subtitle">选择题答案</h3>
        <ol class="answer-list">
"""

    # 添加选择题答案
    for i, answer in enumerate(answers, 1):
        content += f"            <li class='answer-item'>{i}. {answer}</li>\n"

    content += f"""
        </ol>
    </div>
    
    <div class="composition-section">
        <h3 class="section-subtitle">作文</h3>
        <div class="composition-content">
            {composition}
        </div>
    </div>
    
    <div class="questions-section">
        <h3 class="section-subtitle">讨论问题</h3>
        <ol class="question-list">
"""

    # 添加讨论问题
    for i, question in enumerate(questions, 11):
        content += f"            <li class='question-item'>{i}. {question}</li>\n"

    content += f"""
        </ol>
    </div>
</div>
"""

    # 创建自定义CSS样式
    header = """
<style>
    #content{
        width: 500px;
    }
    
    .test-container {
        display: flex;
        flex-direction: column;
        gap: 20px;
    }
    
    .section-title {
        text-align: center;
        padding: 15px;
        background-color: var(--color-primary-container);
        border-radius: 12px;
        margin-bottom: 10px;
    }
    
    .section-title h2 {
        color: var(--color-on-primary-container);
        font-size: 28px;
        margin: 0;
    }
    
    .section-subtitle {
        color: var(--color-primary);
        font-size: 22px;
        margin-bottom: 10px;
        padding-bottom: 8px;
        border-bottom: 2px solid var(--color-outline-variant);
    }
    
    .answers-section, .composition-section, .questions-section {
        background-color: var(--color-surface-container-low);
        padding: 20px;
        border-radius: 12px;
    }
    
    .answer-list, .question-list {
        padding-left: 25px;
        margin: 0;
    }
    
    .answer-item, .question-item {
        font-size: 18px;
        margin-bottom: 8px;
        line-height: 1.6;
    }
    
    .answer-item {
        color: var(--color-on-surface);
    }
    
    .question-item {
        color: var(--color-secondary);
        font-weight: 500;
    }
    
    .composition-content {
        font-size: 18px;
        line-height: 1.8;
        text-align: justify;
        color: var(--color-on-surface);
        background-color: var(--color-surface-container-lowest);
        padding: 15px;
        border-radius: 8px;
        border-left: 4px solid var(--color-tertiary);
    }
</style>
"""

    # 调用htmlToImage函数，保持原有模板的UI风格
    result = await htmlToImage(
        content=content,
        header=header,
        show_logo=True,
        scale=5,
        filename=Path("test_answer.png"),
    )

    logging.info(f"答案渲染完成，图片保存在: {result}")
    return result


def process_text_for_html(text):
    """处理文本以适应HTML显示"""
    # 这里可以添加更多的文本处理逻辑
    return text


def create_basic_html_structure(content):
    """创建基本的HTML结构"""
    return f"<div>{content}</div>"


def create_answers_list(answers):
    """创建答案列表的HTML"""
    items = "".join([f"<li>{answer}</li>" for answer in answers])
    return f"<ol>{items}</ol>"


def create_composition_box(composition):
    """创建作文框的HTML"""
    return f'<div class="composition-box">{composition}</div>'


def create_questions_list(questions):
    """创建问题列表的HTML"""
    items = "".join([f"<li>{question}</li>" for question in questions])
    return f"<ol>{items}</ol>"


def get_custom_css():
    """获取自定义CSS样式"""
    return f"""
    <style></style>
    """


async def main():
    """主函数，运行测试"""
    # 运行答案渲染测试
    await test_answer_display()
    logging.info("所有测试已完成!")
    await close_browser()


if __name__ == "__main__":
    # 运行主函数
    asyncio.run(main())
