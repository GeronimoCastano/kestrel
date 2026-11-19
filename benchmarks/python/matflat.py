def run(w, iterations):
    size = w * w
    a = [0.0] * size
    b = [0.0] * size
    for i in range(w):
        row = i * w
        for j in range(w):
            a[row + j] = float((i * 17 + j * 31) % 100)

    for _ in range(iterations):
        for i in range(1, w - 1):
            row = i * w
            up = row - w
            down = row + w
            for j in range(1, w - 1):
                b[row + j] = 0.25 * (
                    a[up + j] + a[down + j] + a[row + j - 1] + a[row + j + 1]
                )
        b[:w] = a[:w]
        last = (w - 1) * w
        b[last : last + w] = a[last : last + w]
        for i in range(1, w - 1):
            b[i * w] = a[i * w]
            b[i * w + w - 1] = a[i * w + w - 1]
        a, b = b, a

    total = 0.0
    for value in a:
        total += value
    return total


print(f"{run(700, 20):.1f}")
