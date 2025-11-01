def recursive_match(obj, match, array_match_type="all"):
    """
    递归地比较两个嵌套的列表或字典，检查 `match` 是否完全包含在 `obj` 中。

    Args:
        obj (list | dict): 源数据。
        match (list | dict): 匹配数据。
        array_match_type (str): 当比较列表时，指定匹配类型。
                               "all": `match` 列表中的所有元素都必须在 `obj` 列表中，且顺序和长度必须完全相同。
                               "contains": `match` 列表中的所有元素都必须在 `obj` 列表中，且顺序和长度可以不同。

    Returns:
        bool: 如果 `match` 完全包含在 `obj` 中，则返回 True，否则返回 False。
    """
    if isinstance(match, dict):
        if not isinstance(obj, dict):
            return False
        for key, value in match.items():
            if key not in obj:
                return False
            if not recursive_match(obj[key], value, array_match_type):
                return False
        return True

    elif isinstance(match, list):
        if not isinstance(obj, list):
            return False
        if array_match_type == "all":
            if len(obj) != len(match):
                return False
            for i in range(len(match)):
                if not recursive_match(obj[i], match[i], array_match_type):
                    return False
            return True
        
        elif array_match_type == "contains":
            match_copy = match[:]
            for match_item in match_copy:
                found_match = False
                for obj_item in obj:
                    if recursive_match(obj_item, match_item, array_match_type):
                        found_match = True
                        break
                if not found_match:
                    return False
            return True
        else:
            raise ValueError("array_match_type 必须是 'all' 或 'contains'")

    else:
        return obj == match
