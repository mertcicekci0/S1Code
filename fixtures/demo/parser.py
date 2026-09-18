def parse_count(text):
    if not text.strip():
        return 0
    return int(text.strip()[0])
