# std.prob

Probability distributions with a density, a cumulative probability and sampling.

```kestrel
import std.prob
import std.random

let normal := Normal.create(mean = 0.0, sigma = 1.0)
print(normal.pdf(0.0))   // 0.39894228040143265
print(normal.cdf(0.0))   // 0.5

let mut rng := Rng.seeded(1)
print(normal.sample(rng))
```

A distribution is an ordinary value: `create` returns it, and its parameters are
read-only fields with the names in the table below.

## Distributions

| Distribution | Parameters | Kind |
|---|---|---|
| `Normal` | `mean`, `sigma` | continuous |
| `Uniform` | `low`, `high` | continuous |
| `Exponential` | `rate` | continuous |
| `Gamma` | `alpha`, `beta` | continuous |
| `Beta` | `alpha`, `beta` | continuous |
| `ChiSquared` | `dof` | continuous |
| `StudentT` | `dof` | continuous |
| `Cauchy` | `location`, `scale` | continuous |
| `Laplace` | `location`, `scale` | continuous |
| `Logistic` | `location`, `scale` | continuous |
| `Weibull` | `k`, `lambda` | continuous |
| `Lognormal` | `mu`, `sigma` | continuous |
| `Pareto` | `scale`, `alpha` | continuous |
| `Gumbel` | `location`, `scale` | continuous |
| `Rayleigh` | `sigma` | continuous |
| `Triangular` | `low`, `mode`, `high` | continuous |
| `Bernoulli` | `p` | discrete |
| `Binomial` | `trials`, `p` | discrete |
| `Poisson` | `lambda` | discrete |
| `Geometric` | `p` | discrete |
| `NegativeBinomial` | `successes`, `p` | discrete |
| `Hypergeometric` | `population`, `successes`, `draws` | discrete |
| `DiscreteUniform` | `low`, `high` | discrete |

Parameter types follow the value: `double` for a rate or a mean, `Int` for a
count of trials, successes or draws.

## Methods

### `Distribution.create(...) : Distribution`

The static constructor. Every parameter name is required.

| Argument | Meaning |
|---|---|
| the names in the table | The distribution's parameters. |

```kestrel
let poisson := Poisson.create(lambda = 3.0)
let binomial := Binomial.create(trials = 10, p = 0.5)
```

### `pdf(x : double) : double` (continuous)

The probability density at `x`.

| Argument | Meaning |
|---|---|
| `x : double` | The point to evaluate. |

### `pmf(k : Int) : double` (discrete)

The probability mass at `k`.

| Argument | Meaning |
|---|---|
| `k : Int` | The value to evaluate. |

### `logPdf(x : double) : double` / `logPmf(k : Int) : double`

The natural logarithm of the density or mass. It stays finite where the
probability underflows to zero.

### `cdf(x : double) : double` / `cdf(k : Int) : double`

The probability of a value at most `x`, or at most `k`.

```kestrel
print(Poisson.create(lambda = 3.0).cdf(2))
```

### `sample(rng : mut Rng) : double` (continuous) or `Int` (discrete)

Draws one value. The generator is passed by `mut` and advances.

| Argument | Meaning |
|---|---|
| `rng : mut Rng` | The generator to draw from. |

```kestrel
let mut rng := Rng.seeded(42)
let draw := Normal.create(mean = 0.0, sigma = 1.0).sample(rng)
```

## Static forms

Every evaluation method also works as a static call with the parameters passed
explicitly. This is handy for a one-off probability.

```kestrel
print(Normal.pdf(0.0, mean = 0.0, sigma = 1.0))   // 0.39894228040143265
print(Binomial.pmf(5, trials = 10, p = 0.5))      // 0.24609375000000067
```

## Notes

- Parameters are checked where they are used: a negative `sigma` or a
  probability outside `[0, 1]` reports `E_PROB_SHAPE`.
- A continuous distribution evaluates on `double`; a discrete one on `Int`.
- The distributions share the generator with [`std.random`](random.md), so a
  seeded run is reproducible.
