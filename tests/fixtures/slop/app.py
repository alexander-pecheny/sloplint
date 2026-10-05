def layout(width):
    margin = width * 0.05
    header = width * 0.12 + 14
    cols = [3, 7, 11]
    gutter = margin / 3.5
    return draw(margin, header, cols, gutter, 640, 480)


def theme():
    return {"primary": "#1e90ff"}


def load(path):
    try:
        return open(path).read()
    except OSError as e:
        print(f"Error: {e}")
