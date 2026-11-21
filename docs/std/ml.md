# std.ml

Classical machine learning: preprocessing, regression, classification,
clustering, decomposition, splitting and metrics.

```kestrel
import std.ml.{LinearRegression, StandardScaler, meanSquaredError}

let x := Tensor([1.0, 2.0, 2.0, 1.0, 4.0, 3.0, 5.0, 6.0], shape = (4, 2))
let y := Tensor([9.0, 8.0, 18.0, 29.0], shape = (4,))

let scaler := StandardScaler.fit(x)
print(scaler.means)
print(scaler.scales)

let model := LinearRegression.fit(x, y)
print(model.coefficients)
print(model.intercept)
print(meanSquaredError(y, model.predict(x)))
```

The shape of the API is the same everywhere: a static `fit` returns a *fitted*
value, and the fitted value answers `predict` or `transform`. The training data
is not kept.

This is the guide. The [reference](ml-api.md) lists every member of `std.ml` with its signature and arguments.

## Preprocessing

| Class | Fitted value | Methods and fields |
|---|---|---|
| `StandardScaler` | `FittedStandardScaler` | `transform`, `means`, `scales` |
| `MinMaxScaler` | `FittedMinMaxScaler` | `transform`, the recorded minimum and maximum |
| `PolynomialFeatures` | `FittedPolynomialFeatures` | `transform`, the degree used |

`StandardScaler.fit(x)`, `MinMaxScaler.fit(x)` and
`PolynomialFeatures.fit(x, degree)` all return the fitted transformer.
`MinMaxScaler.fitTransform(x)` and `PolynomialFeatures.fitTransform(x, degree)`
skip the intermediate value.

## Regression

| Class | Fitted value | Methods and fields |
|---|---|---|
| `LinearRegression` | `FittedLinearRegression` | `predict`, `coefficients`, `intercept` |
| `LogisticRegression` | `FittedLogisticRegression` | `predict`, the learned weights |

```kestrel
let model := LinearRegression.fit(x, y)
let guess := model.predict(x)
```

`LogisticRegression.fit(x, labels, threshold = 0.5, iterations = 1000)` trains a
binary classifier by gradient descent.

## Classification

| Class | Fitted value | Methods |
|---|---|---|
| `KNeighborsClassifier` | `FittedKNeighborsClassifier` | `predict` |

```kestrel
let neighbors := KNeighborsClassifier.fit(x, labels, 3)
print(neighbors.predict(x))
```

## Clustering and decomposition

| Class | Fitted value | Methods and fields |
|---|---|---|
| `KMeans` | `FittedKMeans` | `fit(x, k, iterations)`, `predict` |
| `PCA` | `FittedPCA` | `fit(x, components)`, `transform`, `explainedVariance` |

```kestrel
let pca := PCA.fit(x, 1)
print(pca.transform(x).shape())
print(pca.explainedVariance)
```

## Splitting

| Function | Meaning |
|---|---|
| `trainTestSplit(x, y, testCount, rng)` | Shuffles and cuts off a test set. |
| `foldIndices(n, folds, index)` | The train and test indices of one cross-validation fold. |

```kestrel
import std.random.{Rng}

let rng := Rng.seeded(7)
let (trainingX, testingX, trainingY, testingY) := trainTestSplit(x, y, 1, rng)
```

## Metrics

| Function | Meaning |
|---|---|
| `meanSquaredError(actual, predicted)` | Mean squared error. |
| `accuracy(actual, predicted)` | Fraction of equal labels. |
| `precision(actual, predicted)` | True positives over predicted positives. |
| `recall(actual, predicted)` | True positives over actual positives. |
| `f1Score(actual, predicted)` | The harmonic mean of precision and recall. |
| `confusionMatrix(actual, predicted)` | Four counts: true negatives, false positives, false negatives, true positives. |

```kestrel
let actual := Tensor([0.0, 0.0, 1.0, 1.0])
let predicted := Tensor([0.0, 1.0, 1.0, 0.0])
print(confusionMatrix(actual, predicted))
print(f1Score(actual, predicted))
```


The complete member list, with every signature and argument, is in the [std.ml reference](ml-api.md).

## Notes

- Labels are doubles, so a binary classification uses `0.0` and `1.0`.
- A feature matrix is rank 2 with one observation per row; a target vector is
  rank 1.
- The models are small and deterministic. For a neural network, use
  [`std.kesnet`](kesnet.md).
