def build(n):
    m = {}
    for i in range(n):
        m[i] = i * 2
    total = sum(m.values())
    items = [v + k for k, v in m.items()]
    return total + sum(items)

print(build(2_000_000))
