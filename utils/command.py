import re
from typing import Dict, List, Tuple


def parse_command_line_args(cmd: str) -> Tuple[List[str], Dict[str, str]]:
    """
    Parses a command-line string into positional arguments and keyword arguments.
    It correctly handles arguments with spaces and special characters enclosed in quotes.
    将命令行字符串解析为位置参数和关键字参数。
    它能正确处理用引号括起来的带有空格和特殊字符的参数。

    Args:
        cmd (str): The command-line string to parse. 要解析的命令行字符串。

    Returns:
        A tuple containing: 一个包含以下内容的元组:
        - A list of positional arguments (strings). 位置参数列表（字符串）。
        - A dictionary of keyword arguments, where keys are argument names and
          values are their string representations. 关键字参数字典，其中键是参数名称，值是其字符串表示。
    """
    args: List[str] = []
    kwargs: Dict[str, str] = {}

    # This regex pattern correctly handles:
    # 1. Key-value pairs (key=value) with or without quotes.
    # 2. Arguments enclosed in double or single quotes.
    # 3. Regular arguments without quotes.
    # 此正则表达式模式能正确处理:
    # 1. 带或不带引号的键值对（key=value）。
    # 2. 用双引号或单引号括起来的参数。
    # 3. 不带引号的普通参数。
    pattern = re.compile(
        r'(\w+)=(?:"([^"]*)"|\'([^\']*)\'|([^\s]+))|(?:"([^"]*)"|\'([^\']*)\'|([^\s]+))'
    )

    for match in re.finditer(pattern, cmd):
        # Check for a key-value pair
        # 检查是否为键值对
        if match.group(1):
            key = match.group(1)
            # The value could be in group 2 (double quotes), 3 (single quotes), or 4 (unquoted)
            # 值可能在第 2 组（双引号）、第 3 组（单引号）或第 4 组（不带引号）
            value = match.group(2) or match.group(3) or match.group(4)
            kwargs[key] = value
        else:
            # It's a positional argument
            # 这是位置参数
            value = match.group(5) or match.group(6) or match.group(7)
            if value:
                args.append(value)

    return args, kwargs
