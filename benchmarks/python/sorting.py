values = []
x = 42
for _ in range(2_000_000):
    x = (x * 1103515245 + 12345) % 2147483648
    values.append(x)
values.sort()
print(sum(values[::1000]))
