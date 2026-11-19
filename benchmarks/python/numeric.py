def lcg(n):
    x, total = 42, 0
    for _ in range(n):
        x = (x * 1103515245 + 12345) % 2147483648
        total += x % 1000
    return total

def leibniz(n):
    total, sign = 0.0, 1.0
    for i in range(n):
        total += sign / (2.0 * i + 1.0)
        sign = -sign
    return 4.0 * total

N = 50_000_000
print(lcg(N))
print(f"{leibniz(N):.9f}")
