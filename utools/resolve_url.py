import aiohttp


async def resolve_url(url):
    async with aiohttp.ClientSession() as session:
        async with session.get(url, allow_redirects=False) as response:
            if response.status == 301 or response.status == 302:
                return response.headers.get("Location")
            return url
