factorial = 1
for i in range(2, 100_001):
    factorial *= i
print(factorial % 1000000007)
