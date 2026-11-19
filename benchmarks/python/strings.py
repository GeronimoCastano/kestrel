def build(n):
    parts = []
    for i in range(n):
        parts.append(str(i))
        parts.append(",")
    return len("".join(parts))

print(build(2_000_000))
