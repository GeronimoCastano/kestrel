# std.ml reference

Part of [`std.ml`](ml.md); this page lists its members, the guide explains the workflow.

Every public member of `std.ml`. The path of each member is written from the
package root, as in `ml.LinearRegression.fit`; after `import std.ml` the
namespace is `ml`.

A model is fitted with a static `fit`, and the fitted value answers `predict` or
`transform`.

```kestrel
let scaler := StandardScaler.fit(x)
let model := LinearRegression.fit(scaler.transform(x), y)
let score := accuracy(y, model.predict(scaler.transform(x)))
```


### Types

- `ml.FittedStandardScaler`: A fitted standardiser: transform new data with its statistics.
- `ml.StandardScaler`: Standardises features to zero mean and unit variance.
- `ml.FittedLinearRegression`: A fitted linear model with coefficients and an intercept.
- `ml.LinearRegression`: Ordinary least squares.
- `ml.FittedMinMaxScaler`: A fitted range scaler.
- `ml.MinMaxScaler`: Scales features into a range.
- `ml.FittedPolynomialFeatures`: A fitted polynomial expansion.
- `ml.PolynomialFeatures`: Expands features into polynomial terms.
- `ml.FittedKNeighborsClassifier`: A fitted nearest-neighbour model.
- `ml.KNeighborsClassifier`: K-nearest-neighbours classification.
- `ml.FittedLogisticRegression`: A fitted logistic model.
- `ml.LogisticRegression`: Binary logistic regression by gradient descent.
- `ml.FittedKMeans`: A fitted k-means model.
- `ml.KMeans`: K-means clustering.
- `ml.FittedPCA`: A fitted projection with its explained variance.
- `ml.PCA`: Principal component analysis.

### FittedStandardScaler

| Member | Signature | What it does |
|---|---|---|
| `ml.FittedStandardScaler.transform` | `FittedStandardScaler.transform(X : DynTensor[double]) : DynTensor[double]` | Transforms new data with the fitted parameters. |

### Functions

| Member | Signature | What it does |
|---|---|---|
| `ml.StandardScaler.fit` | `StandardScaler.fit(X : DynTensor[double]) : FittedStandardScaler` | Trains the model and returns a fitted value. |
| `ml.StandardScaler.fitTransform` | `StandardScaler.fitTransform(X : DynTensor[double]) : DynTensor[double]` | Fits on the data and transforms it in one step. |
| `ml.LinearRegression.fit` | `LinearRegression.fit(X : DynTensor[double], y : DynTensor[double], fitIntercept : Bool) : FittedLinearRegression` | Trains the model and returns a fitted value. |
| `ml.meanSquaredError` | `std.ml.meanSquaredError(actual : DynTensor[double], predicted : DynTensor[double]) : double` | The mean squared error. |
| `ml.meanAbsoluteError` | `std.ml.meanAbsoluteError(actual : DynTensor[double], predicted : DynTensor[double]) : double` | The mean absolute error. |
| `ml.r2Score` | `std.ml.r2Score(actual : DynTensor[double], predicted : DynTensor[double]) : double` | The coefficient of determination, $R^{2}$. |
| `ml.accuracy` | `std.ml.accuracy(actual : DynTensor[double], predicted : DynTensor[double]) : double` | The fraction of predictions that equal the labels. |
| `ml.MinMaxScaler.fit` | `MinMaxScaler.fit(X : DynTensor[double]) : FittedMinMaxScaler` | Trains the model and returns a fitted value. |
| `ml.MinMaxScaler.fitTransform` | `MinMaxScaler.fitTransform(X : DynTensor[double]) : DynTensor[double]` | Fits on the data and transforms it in one step. |
| `ml.PolynomialFeatures.fit` | `PolynomialFeatures.fit(X : DynTensor[double], bias : Bool) : FittedPolynomialFeatures` | Trains the model and returns a fitted value. |
| `ml.PolynomialFeatures.fitTransform` | `PolynomialFeatures.fitTransform(X : DynTensor[double], bias : Bool) : DynTensor[double]` | Fits on the data and transforms it in one step. |
| `ml.shuffleIndices` | `std.ml.shuffleIndices(count : Int, rng : Rng) : Array[Int]` | The positions in random order. |
| `ml.sliceIndices` | `std.ml.sliceIndices(indices : Array[Int], start : Int, stop : Int) : Array[Int]` | The values at a range of positions. |
| `ml.takeRows` | `std.ml.takeRows(X : DynTensor[double], indices : Array[Int]) : DynTensor[double]` | The rows at the given positions. |
| `ml.takeValues` | `std.ml.takeValues(y : DynTensor[double], indices : Array[Int]) : DynTensor[double]` | The values at the given positions. |
| `ml.trainTestSplit` | `std.ml.trainTestSplit(X : DynTensor[double], y : DynTensor[double], testCount : Int, rng : Rng) : (DynTensor[double], DynTensor[double], DynTensor[double], DynTensor[double])` | Splits a dataset into training and test parts. |
| `ml.KNeighborsClassifier.fit` | `KNeighborsClassifier.fit(X : DynTensor[double], y : DynTensor[double], neighbors : Int) : FittedKNeighborsClassifier` | Trains the model and returns a fitted value. |
| `ml.LogisticRegression.fit` | `LogisticRegression.fit(X : DynTensor[double], y : DynTensor[double], learningRate : double, iterations : Int, fitIntercept : Bool, solver : String) : FittedLogisticRegression` | Trains the model and returns a fitted value. |
| `ml.KMeans.fit` | `KMeans.fit(X : DynTensor[double], clusters : Int, iterations : Int) : FittedKMeans` | Trains the model and returns a fitted value. |
| `ml.PCA.fit` | `PCA.fit(X : DynTensor[double], nComponents : Int) : FittedPCA` | Trains the model and returns a fitted value. |
| `ml.foldIndices` | `std.ml.foldIndices(count : Int, folds : Int, fold : Int) : (Array[Int], Array[Int])` | The train and test positions of one cross-validation fold. |
| `ml.confusionMatrix` | `std.ml.confusionMatrix(actual : DynTensor[double], predicted : DynTensor[double]) : (double, double, double, double)` | The four counts of a binary classifier. |
| `ml.precision` | `std.ml.precision(actual : DynTensor[double], predicted : DynTensor[double]) : double` | True positives over predicted positives. |
| `ml.recall` | `std.ml.recall(actual : DynTensor[double], predicted : DynTensor[double]) : double` | True positives over actual positives. |
| `ml.f1Score` | `std.ml.f1Score(actual : DynTensor[double], predicted : DynTensor[double]) : double` | The harmonic mean of precision and recall. |

### FittedLinearRegression

| Member | Signature | What it does |
|---|---|---|
| `ml.FittedLinearRegression.predict` | `FittedLinearRegression.predict(X : DynTensor[double]) : DynTensor[double]` | Predicts for each row. |

### FittedMinMaxScaler

| Member | Signature | What it does |
|---|---|---|
| `ml.FittedMinMaxScaler.transform` | `FittedMinMaxScaler.transform(X : DynTensor[double]) : DynTensor[double]` | Transforms new data with the fitted parameters. |

### FittedPolynomialFeatures

| Member | Signature | What it does |
|---|---|---|
| `ml.FittedPolynomialFeatures.transform` | `FittedPolynomialFeatures.transform(X : DynTensor[double]) : DynTensor[double]` | Transforms new data with the fitted parameters. |

### FittedKNeighborsClassifier

| Member | Signature | What it does |
|---|---|---|
| `ml.FittedKNeighborsClassifier.predict` | `FittedKNeighborsClassifier.predict(X : DynTensor[double]) : DynTensor[double]` | Predicts for each row. |

### FittedLogisticRegression

| Member | Signature | What it does |
|---|---|---|
| `ml.FittedLogisticRegression.predictProba` | `FittedLogisticRegression.predictProba(X : DynTensor[double]) : DynTensor[double]` | The predicted probability for each row. |
| `ml.FittedLogisticRegression.predict` | `FittedLogisticRegression.predict(X : DynTensor[double]) : DynTensor[double]` | Predicts for each row. |

### FittedKMeans

| Member | Signature | What it does |
|---|---|---|
| `ml.FittedKMeans.predict` | `FittedKMeans.predict(X : DynTensor[double]) : DynTensor[double]` | Predicts for each row. |

### FittedPCA

| Member | Signature | What it does |
|---|---|---|
| `ml.FittedPCA.transform` | `FittedPCA.transform(X : DynTensor[double]) : DynTensor[double]` | Transforms new data with the fitted parameters. |
