n = 5_000_000
values = []
for i in range(n):
    values.append(i % 1000)

total = 0
for value in values:
    total += value

for _ in range(n // 2):
    total += values.pop()

print(total)
