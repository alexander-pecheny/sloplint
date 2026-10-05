def ladder(x, y):
    if x == 1:
        return "a"
    elif x == 2:
        return "b"
    elif x == 3 and y:
        return "c"
    elif x == 4:
        return "d"
    elif x == 5:
        return "e"
    else:
        return "f"


def nested(items):
    for i in items:
        if i:
            while i > 0:
                try:
                    i -= 1
                except ValueError:
                    pass
    return items
