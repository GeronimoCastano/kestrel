# The Benchmarks Game binary-trees program, single-threaded.
def make(depth):
    return (None, None) if depth == 0 else (make(depth - 1), make(depth - 1))

def check(t):
    left, right = t
    return 1 + (0 if left is None else check(left) + check(right))

max_depth = 18
stretch = max_depth + 1
print(f"stretch tree of depth {stretch}\t check: {check(make(stretch))}")
long_lived = make(max_depth)
for depth in range(4, max_depth + 1, 2):
    iterations = 1 << (max_depth - depth + 4)
    total = sum(check(make(depth)) for _ in range(iterations))
    print(f"{iterations}\t trees of depth {depth}\t check: {total}")
print(f"long lived tree of depth {max_depth}\t check: {check(long_lived)}")
