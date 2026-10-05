MARGIN_RATIO = 0.05


def layout(width, theme):
    return draw(width * MARGIN_RATIO, theme.primary)


def load(path):
    try:
        return open(path).read()
    except OSError as e:
        raise ConfigError(path) from e


# sloplint: ignore[hardcoded-color]
FALLBACK = "#000000"
