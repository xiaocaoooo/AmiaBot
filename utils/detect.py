import aiohttp


async def detect(text: str) -> bool:
    """
    检测文本是否包含敏感内容
    :param text: 要检测的文本
    :return: 如果文本包含敏感内容，则返回True；否则返回False
    """
    # GET https://v2.xxapi.cn/api/detect?text={text}
    url = "https://v2.xxapi.cn/api/detect"
    params = {"text": text}
    async with aiohttp.ClientSession() as session:
        async with session.get(url, params=params) as resp:
            data = await resp.json()
            if data["code"] == 200:
                return data["data"]["confidence"] > 0.9
            else:
                return False
