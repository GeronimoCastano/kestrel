N = 5_000_000

evens = [x for x in range(N) if x % 2 == 0]
doubled = [x * 2 for x in evens]
shifted = [x + 7 for x in doubled]
kept = [x for x in shifted if x % 3 == 0]
print(sum(kept))
