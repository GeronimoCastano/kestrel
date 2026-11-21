# std.ml reference

Every public member of `std.ml`. A model is fitted with a static `fit`, and the
fitted value answers `predict` or `transform`.

```kestrel
let scaler := StandardScaler.fit(x)
let model := LinearRegression.fit(scaler.transform(x), y)
let score := accuracy(y, model.predict(scaler.transform(x)))
```


### Types

- `FittedStandardScaler`: A fitted standardiser: transform new data with its statistics.
- `StandardScaler`: Standardises features to zero mean and unit variance.
- `FittedLinearRegression`: A fitted linear model with coefficients and an intercept.
- `LinearRegression`: Ordinary least squares.
- `FittedMinMaxScaler`: A fitted range scaler.
- `MinMaxScaler`: Scales features into a range.
- `FittedPolynomialFeatures`: A fitted polynomial expansion.
- `PolynomialFeatures`: Expands features into polynomial terms.
- `FittedKNeighborsClassifier`: A fitted nearest-neighbour model.
- `KNeighborsClassifier`: K-nearest-neighbours classification.
- `FittedLogisticRegression`: A fitted logistic model.
- `LogisticRegression`: Binary logistic regression by gradient descent.
- `FittedKMeans`: A fitted k-means model.
- `KMeans`: K-means clustering.
- `FittedPCA`: A fitted projection with its explained variance.
- `PCA`: Principal component analysis.

### FittedStandardScaler

| Member | Signature | What it does |
|---|---|---|
| `transform` | `FittedStandardScaler.transform(X : DynTensor[double]) : DynTensor[double]` | Transforms new data with the fitted parameters. |

### Functions

| Member | Signature | What it does |
|---|---|---|
| `fit` | `StandardScaler.fit(X : DynTensor[double]) : FittedStandardScaler` | Trains the model and returns a fitted value. |
| `fitTransform` | `StandardScaler.fitTransform(X : DynTensor[double]) : DynTensor[double]` | Fits on the data and transforms it in one step. |
| `fit` | `LinearRegression.fit(X : DynTensor[double], y : DynTensor[double], fitIntercept : Bool) : FittedLinearRegression` | Trains the model and returns a fitted value. |
| `meanSquaredError` | `std.ml.meanSquaredError(actual : DynTensor[double], predicted : DynTensor[double]) : double` | The mean squared error. |
| `meanAbsoluteError` | `std.ml.meanAbsoluteError(actual : DynTensor[double], predicted : DynTensor[double]) : double` | The mean absolute error. |
| `r2Score` | `std.ml.r2Score(actual : DynTensor[double], predicted : DynTensor[double]) : double` | The coefficient of determination, $R^{2}$. |
| `accuracy` | `std.ml.accuracy(actual : DynTensor[double], predicted : DynTensor[double]) : double` | The fraction of predictions that equal the labels. |
| `fit` | `MinMaxScaler.fit(X : DynTensor[double]) : FittedMinMaxScaler` | Trains the model and returns a fitted value. |
| `fitTransform` | `MinMaxScaler.fitTransform(X : DynTensor[double]) : DynTensor[double]` | Fits on the data and transforms it in one step. |
| `fit` | `PolynomialFeatures.fit(X : DynTensor[double], bias : Bool) : FittedPolynomialFeatures` | Trains the model and returns a fitted value. |
| `fitTransform` | `PolynomialFeatures.fitTransform(X : DynTensor[double], bias : Bool) : DynTensor[double]` | Fits on the data and transforms it in one step. |
| `shuffleIndices` | `std.ml.shuffleIndices(count : Int, rng : Rng) : Array[Int]` | The positions in random order. |
| `sliceIndices` | `std.ml.sliceIndices(indices : Array[Int], start : Int, stop : Int) : Array[Int]` | The values at a range of positions. |
| `takeRows` | `std.ml.takeRows(X : DynTensor[double], indices : Array[Int]) : DynTensor[double]` | The rows at the given positions. |
| `takeValues` | `std.ml.takeValues(y : DynTensor[double], indices : Array[Int]) : DynTensor[double]` | The values at the given positions. |
| `trainTestSplit` | `std.ml.trainTestSplit(X : DynTensor[double], y : DynTensor[double], testCount : Int, rng : Rng) : (DynTensor[double], DynTensor[double], DynTensor[double], DynTensor[double])` | Splits a dataset into training and test parts. |
| `fit` | `KNeighborsClassifier.fit(X : DynTensor[double], y : DynTensor[double], neighbors : Int) : FittedKNeighborsClassifier` | Trains the model and returns a fitted value. |
| `fit` | `LogisticRegression.fit(X : DynTensor[double], y : DynTensor[double], learningRate : double, iterations : Int, fitIntercept : Bool, solver : String) : FittedLogisticRegression` | Trains the model and returns a fitted value. |
| `fit` | `KMeans.fit(X : DynTensor[double], clusters : Int, iterations : Int) : FittedKMeans` | Trains the model and returns a fitted value. |
| `fit` | `PCA.fit(X : DynTensor[double], nComponents : Int) : FittedPCA` | Trains the model and returns a fitted value. |
| `foldIndices` | `std.ml.foldIndices(count : Int, folds : Int, fold : Int) : (Array[Int], Array[Int])` | The train and test positions of one cross-validation fold. |
| `confusionMatrix` | `std.ml.confusionMatrix(actual : DynTensor[double], predicted : DynTensor[double]) : (double, double, double, double)` | The four counts of a binary classifier. |
| `precision` | `std.ml.precision(actual : DynTensor[double], predicted : DynTensor[double]) : double` | True positives over predicted positives. |
| `recall` | `std.ml.recall(actual : DynTensor[double], predicted : DynTensor[double]) : double` | True positives over actual positives. |
| `f1Score` | `std.ml.f1Score(actual : DynTensor[double], predicted : DynTensor[double]) : double` | The harmonic mean of precision and recall. |

### FittedLinearRegression

| Member | Signature | What it does |
|---|---|---|
| `predict` | `FittedLinearRegression.predict(X : DynTensor[double]) : DynTensor[double]` | Predicts for each row. |

### FittedMinMaxScaler

| Member | Signature | What it does |
|---|---|---|
| `transform` | `FittedMinMaxScaler.transform(X : DynTensor[double]) : DynTensor[double]` | Transforms new data with the fitted parameters. |

### FittedPolynomialFeatures

| Member | Signature | What it does |
|---|---|---|
| `transform` | `FittedPolynomialFeatures.transform(X : DynTensor[double]) : DynTensor[double]` | Transforms new data with the fitted parameters. |

### FittedKNeighborsClassifier

| Member | Signature | What it does |
|---|---|---|
| `predict` | `FittedKNeighborsClassifier.predict(X : DynTensor[double]) : DynTensor[double]` | Predicts for each row. |

### FittedLogisticRegression

| Member | Signature | What it does |
|---|---|---|
| `predictProba` | `FittedLogisticRegression.predictProba(X : DynTensor[double]) : DynTensor[double]` | The predicted probability for each row. |
| `predict` | `FittedLogisticRegression.predict(X : DynTensor[double]) : DynTensor[double]` | Predicts for each row. |

### FittedKMeans

| Member | Signature | What it does |
|---|---|---|
| `predict` | `FittedKMeans.predict(X : DynTensor[double]) : DynTensor[double]` | Predicts for each row. |

### FittedPCA

| Member | Signature | What it does |
|---|---|---|
| `transform` | `FittedPCA.transform(X : DynTensor[double]) : DynTensor[double]` | Transforms new data with the fitted parameters. |
