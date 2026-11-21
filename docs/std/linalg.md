# std.linalg

Vectors and matrices as tensors, with products, solves and decompositions.

```kestrel
import std.linalg

let a := Tensor([4.0, 2.0, 2.0, 3.0], shape = (2, 2))
print(linalg.det(a))        // 8.0
print(linalg.norm(a))       // 5.7445626465380286
print(linalg.inverse(a))
```

```text
[[0.375, -0.25],
 [-0.25, 0.5]]
```

Build a tensor with `Tensor(values, shape = (rows, columns))`, row by row. A
vector is a tensor of shape `(n,)` or `(n, 1)`.

## Products and norms

| Function | Returns | Meaning |
|---|---|---|
| `dot(a, b)` | `double` | The sum of the elementwise products. |
| `outer(a, b)` | `Tensor` | The outer product of two vectors. |
| `norm(a)` | `double` | The Euclidean norm of the entries. |
| `frobeniusNorm(a)` | `double` | The same for a matrix, named for clarity. |
| `spectralNorm(a)` | `double` | The largest singular value. |

| Argument | Meaning |
|---|---|
| `a : Tensor` | The first operand. |
| `b : Tensor` | The second operand. |

```kestrel
let x := Tensor([1.0, 2.0, 3.0], shape = (3,))
let y := Tensor([4.0, 5.0, 6.0], shape = (3,))
print(linalg.dot(x, y))   // 32.0
```

## Solving

| Function | Returns | Meaning |
|---|---|---|
| `solve(a, b)` | `Tensor` | Solves $a x = b$ for a square `a`. |
| `solveLower(a, b)` | `Tensor` | The same for a lower-triangular `a`. |
| `solveUpper(a, b)` | `Tensor` | The same for an upper-triangular `a`. |
| `leastSquares(a, b)` | `Tensor` | The least-squares solution for a tall or square `a`. |

| Argument | Meaning |
|---|---|
| `a : Tensor` | The coefficient matrix. |
| `b : Tensor` | The right-hand side. |

```kestrel
let a := Tensor([4.0, 2.0, 2.0, 3.0], shape = (2, 2))
let b := Tensor([1.0, 2.0], shape = (2, 1))
print(linalg.solve(a, b))
```

```text
[[-0.125],
 [0.75]]
```

## Decompositions

| Function | Returns | Fields |
|---|---|---|
| `lu(a)` | `LU` | `factors`, `pivots`, and `solve(b)` |
| `qr(a)` | `QR` | `q`, `r` |
| `cholesky(a)` | `Tensor` | The lower factor $L$ with $A = L L^{T}$ |
| `svd(a)` | `SVD` | `u`, `s`, `vt`; singular values in `s`, descending |
| `eigh(a)` | `Eigh` | `values` ascending, `vectors` |

```kestrel
let lu := linalg.lu(a)
print(lu.factors)          // the packed factors
print(lu.solve(b))         // the same answer as linalg.solve
```

`eigh` expects a symmetric matrix. Its eigenvalues come out ascending, the
opposite order to `svd`'s singular values.

## Shape and rank

| Function | Returns | Meaning |
|---|---|---|
| `det(a)` | `double` | The determinant. |
| `rank(a)` | `Int` | The numerical rank. |
| `inverse(a)` | `Tensor` | The inverse, for a square nonsingular matrix. |
| `pinv(a)` | `Tensor` | The Moore-Penrose pseudoinverse. |

| Argument | Meaning |
|---|---|
| `a : Tensor` | The matrix. |

```kestrel
print(linalg.rank(a))   // 2
```

## Notes

- LAPACK or OpenBLAS does the work; the library is loaded when a program first
  needs it. A missing library stops the program with a message that names it.
- A singular matrix in `solve` or `inverse` stops with a diagnostic instead of
  returning infinities.
- Decompositions return records, so the fields are read as `svd.s`, not called.
