'use strict';

// Metadata for std.ml. The compiler remains the authority; this drives
// completion, hover and result-type inference.
const types = [
  'StandardScaler', 'FittedStandardScaler',
  'MinMaxScaler', 'FittedMinMaxScaler',
  'PolynomialFeatures', 'FittedPolynomialFeatures',
  'LinearRegression', 'FittedLinearRegression',
  'KNeighborsClassifier', 'FittedKNeighborsClassifier',
  'LogisticRegression', 'FittedLogisticRegression',
  'KMeans', 'FittedKMeans',
  'PCA', 'FittedPCA'
];

const names = [
  ...types,
  'shuffleIndices', 'sliceIndices', 'takeRows', 'takeValues',
  'trainTestSplit', 'foldIndices',
  'meanSquaredError', 'meanAbsoluteError', 'r2Score', 'accuracy',
  'confusionMatrix', 'precision', 'recall', 'f1Score'
];

const entries = [];
function typeEntry(name, documentation) {
  entries.push({ name, module: 'std.ml', gated: true, type: true, signature: name, parameters: [], documentation });
}
function staticEntry(owner, name, signature, parameters, documentation) {
  entries.push({ name, module: 'std.ml', gated: true, static: owner, signature, parameters, documentation });
}
function methodEntry(receiver, name, signature, parameters, documentation) {
  entries.push({ name, module: 'std.ml', gated: true, receiver, signature, parameters, documentation });
}
function fieldEntry(receiver, name, type, documentation) {
  entries.push({ name, module: 'std.ml', gated: true, receiver, field: true, signature: `${receiver}.${name} : ${type}`, parameters: [], documentation });
}
function functionEntry(name, signature, parameters, documentation) {
  entries.push({ name, module: 'std.ml', gated: true, signature, parameters, documentation });
}

for (const name of types) typeEntry(name, 'Imported machine-learning type.');

staticEntry('StandardScaler', 'fit', 'StandardScaler.fit(X : DynTensor[double]) : FittedStandardScaler', ['X : DynTensor[double]'], 'Computes per-feature means and population standard deviations.');
staticEntry('StandardScaler', 'fitTransform', 'StandardScaler.fitTransform(X : DynTensor[double]) : DynTensor[double]', ['X : DynTensor[double]'], 'Fits the scaler and transforms the same features.');
staticEntry('MinMaxScaler', 'fit', 'MinMaxScaler.fit(X : DynTensor[double]) : FittedMinMaxScaler', ['X : DynTensor[double]'], 'Computes per-feature minimums and ranges.');
staticEntry('MinMaxScaler', 'fitTransform', 'MinMaxScaler.fitTransform(X : DynTensor[double]) : DynTensor[double]', ['X : DynTensor[double]'], 'Fits the scaler and transforms the same features.');
staticEntry('PolynomialFeatures', 'fit', 'PolynomialFeatures.fit(X : DynTensor[double], bias : Bool = false) : FittedPolynomialFeatures', ['X : DynTensor[double]', 'bias : Bool = false'], 'Prepares the degree-two expansion: optional bias, originals, squares and pairwise products.');
staticEntry('PolynomialFeatures', 'fitTransform', 'PolynomialFeatures.fitTransform(X : DynTensor[double], bias : Bool = false) : DynTensor[double]', ['X : DynTensor[double]', 'bias : Bool = false'], 'Expands the features through degree two, optionally with a bias column.');
staticEntry('LinearRegression', 'fit', 'LinearRegression.fit(X : DynTensor[double], y : DynTensor[double], fitIntercept : Bool = true) : FittedLinearRegression', ['X : DynTensor[double]', 'y : DynTensor[double]', 'fitIntercept : Bool = true'], 'Fits ordinary least squares, centered for an intercept by default.');
staticEntry('KNeighborsClassifier', 'fit', 'KNeighborsClassifier.fit(X : DynTensor[double], y : DynTensor[double], neighbors : Int = 5) : FittedKNeighborsClassifier', ['X : DynTensor[double]', 'y : DynTensor[double]', 'neighbors : Int = 5'], 'Copies the training rows and labels for a k-nearest-neighbor vote.');
staticEntry('LogisticRegression', 'fit', 'LogisticRegression.fit(X : DynTensor[double], y : DynTensor[double], learningRate : double = 0.1, iterations : Int = 1000, fitIntercept : Bool = true, solver : String = "auto") : FittedLogisticRegression', ['X : DynTensor[double]', 'y : DynTensor[double]', 'learningRate : double = 0.1', 'iterations : Int = 1000', 'fitIntercept : Bool = true', 'solver : String = "auto"'], 'Fits binary logistic regression, without automatic differentiation. The default Newton solver handles up to 256 features; gradient keeps the explicit batch gradient loop.');
staticEntry('KMeans', 'fit', 'KMeans.fit(X : DynTensor[double], clusters : Int = 3, iterations : Int = 100) : FittedKMeans', ['X : DynTensor[double]', 'clusters : Int = 3', 'iterations : Int = 100'], 'Runs deterministic k-means from the first rows as initial centers.');
staticEntry('PCA', 'fit', 'PCA.fit(X : DynTensor[double], nComponents : Int = 2) : FittedPCA', ['X : DynTensor[double]', 'nComponents : Int = 2'], 'Centers the rows and keeps the leading principal directions.');

fieldEntry('FittedStandardScaler', 'means', 'DynTensor[double]', 'Per-feature means.');
fieldEntry('FittedStandardScaler', 'scales', 'DynTensor[double]', 'Per-feature scales; a zero-variance feature uses 1.0.');
methodEntry('FittedStandardScaler', 'transform', 'FittedStandardScaler.transform(X : DynTensor[double]) : DynTensor[double]', ['X : DynTensor[double]'], 'Centers and scales new rows with the fitted parameters.');
fieldEntry('FittedMinMaxScaler', 'minimums', 'DynTensor[double]', 'Per-feature minimums.');
fieldEntry('FittedMinMaxScaler', 'ranges', 'DynTensor[double]', 'Per-feature ranges; a constant feature uses 1.0.');
methodEntry('FittedMinMaxScaler', 'transform', 'FittedMinMaxScaler.transform(X : DynTensor[double]) : DynTensor[double]', ['X : DynTensor[double]'], 'Scales new rows to the fitted minimum and range.');
fieldEntry('FittedPolynomialFeatures', 'inputFeatures', 'Int', 'Number of input columns.');
fieldEntry('FittedPolynomialFeatures', 'includeBias', 'Bool', 'Whether transform puts a ones column first.');
methodEntry('FittedPolynomialFeatures', 'transform', 'FittedPolynomialFeatures.transform(X : DynTensor[double]) : DynTensor[double]', ['X : DynTensor[double]'], 'Expands each row through degree two, with a bias column when fitted for one.');
fieldEntry('FittedLinearRegression', 'coefficients', 'DynTensor[double]', 'One coefficient per feature.');
fieldEntry('FittedLinearRegression', 'intercept', 'double', 'The fitted intercept, or 0.0 when fitIntercept is false.');
methodEntry('FittedLinearRegression', 'predict', 'FittedLinearRegression.predict(X : DynTensor[double]) : DynTensor[double]', ['X : DynTensor[double]'], 'Predicts one value per row.');
fieldEntry('FittedKNeighborsClassifier', 'trainingX', 'DynTensor[double]', 'The copied training rows.');
fieldEntry('FittedKNeighborsClassifier', 'trainingY', 'DynTensor[double]', 'The copied training labels.');
fieldEntry('FittedKNeighborsClassifier', 'neighbors', 'Int', 'The number of neighbors used by the vote.');
methodEntry('FittedKNeighborsClassifier', 'predict', 'FittedKNeighborsClassifier.predict(X : DynTensor[double]) : DynTensor[double]', ['X : DynTensor[double]'], 'Predicts the majority label among the nearest training rows.');
fieldEntry('FittedLogisticRegression', 'coefficients', 'DynTensor[double]', 'One coefficient per feature.');
fieldEntry('FittedLogisticRegression', 'intercept', 'double', 'The fitted intercept.');
methodEntry('FittedLogisticRegression', 'predictProba', 'FittedLogisticRegression.predictProba(X : DynTensor[double]) : DynTensor[double]', ['X : DynTensor[double]'], 'Predicts the probability of class 1.');
methodEntry('FittedLogisticRegression', 'predict', 'FittedLogisticRegression.predict(X : DynTensor[double]) : DynTensor[double]', ['X : DynTensor[double]'], 'Predicts 0.0 or 1.0 at the 0.5 threshold.');
fieldEntry('FittedKMeans', 'centers', 'DynTensor[double]', 'The fitted cluster centers.');
methodEntry('FittedKMeans', 'predict', 'FittedKMeans.predict(X : DynTensor[double]) : DynTensor[double]', ['X : DynTensor[double]'], 'Returns the nearest cluster for each row.');
fieldEntry('FittedPCA', 'means', 'DynTensor[double]', 'Per-feature means.');
fieldEntry('FittedPCA', 'components', 'DynTensor[double]', 'Leading principal directions, one per row.');
fieldEntry('FittedPCA', 'explainedVariance', 'DynTensor[double]', 'Variance carried by each retained component.');
methodEntry('FittedPCA', 'transform', 'FittedPCA.transform(X : DynTensor[double]) : DynTensor[double]', ['X : DynTensor[double]'], 'Projects rows onto the retained components.');

functionEntry('shuffleIndices', 'shuffleIndices(count : Int, mut rng : Rng) : Array[Int]', ['count : Int', 'rng : Rng'], 'Returns a shuffled range of indices.');
functionEntry('sliceIndices', 'sliceIndices(indices : Array[Int], start : Int, stop : Int) : Array[Int]', ['indices : Array[Int]', 'start : Int', 'stop : Int'], 'Copies a half-open index range.');
functionEntry('takeRows', 'takeRows(X : DynTensor[double], indices : Array[Int]) : DynTensor[double]', ['X : DynTensor[double]', 'indices : Array[Int]'], 'Copies the selected rows into a new tensor.');
functionEntry('takeValues', 'takeValues(y : DynTensor[double], indices : Array[Int]) : DynTensor[double]', ['y : DynTensor[double]', 'indices : Array[Int]'], 'Copies the selected values into a new tensor.');
functionEntry('trainTestSplit', 'trainTestSplit(X : DynTensor[double], y : DynTensor[double], testCount : Int, mut rng : Rng) : (DynTensor[double], DynTensor[double], DynTensor[double], DynTensor[double])', ['X : DynTensor[double]', 'y : DynTensor[double]', 'testCount : Int', 'rng : Rng'], 'Shuffles rows and returns (trainingX, testingX, trainingY, testingY).');
functionEntry('foldIndices', 'foldIndices(count : Int, folds : Int, fold : Int) : (Array[Int], Array[Int])', ['count : Int', 'folds : Int', 'fold : Int'], 'Returns the training and testing indices for one fold.');
functionEntry('meanSquaredError', 'meanSquaredError(actual : DynTensor[double], predicted : DynTensor[double]) : double', ['actual : DynTensor[double]', 'predicted : DynTensor[double]'], 'Mean of the squared residuals.');
functionEntry('meanAbsoluteError', 'meanAbsoluteError(actual : DynTensor[double], predicted : DynTensor[double]) : double', ['actual : DynTensor[double]', 'predicted : DynTensor[double]'], 'Mean of the absolute residuals.');
functionEntry('r2Score', 'r2Score(actual : DynTensor[double], predicted : DynTensor[double]) : double', ['actual : DynTensor[double]', 'predicted : DynTensor[double]'], 'Coefficient of determination.');
functionEntry('accuracy', 'accuracy(actual : DynTensor[double], predicted : DynTensor[double]) : double', ['actual : DynTensor[double]', 'predicted : DynTensor[double]'], 'Fraction of equal labels.');
functionEntry('confusionMatrix', 'confusionMatrix(actual : DynTensor[double], predicted : DynTensor[double]) : (double, double, double, double)', ['actual : DynTensor[double]', 'predicted : DynTensor[double]'], 'Returns (trueNegative, falsePositive, falseNegative, truePositive) for binary labels.');
functionEntry('precision', 'precision(actual : DynTensor[double], predicted : DynTensor[double]) : double', ['actual : DynTensor[double]', 'predicted : DynTensor[double]'], 'Binary precision, or 0.0 when the predicted-positive count is zero.');
functionEntry('recall', 'recall(actual : DynTensor[double], predicted : DynTensor[double]) : double', ['actual : DynTensor[double]', 'predicted : DynTensor[double]'], 'Binary recall, or 0.0 when the actual-positive count is zero.');
functionEntry('f1Score', 'f1Score(actual : DynTensor[double], predicted : DynTensor[double]) : double', ['actual : DynTensor[double]', 'predicted : DynTensor[double]'], 'Harmonic mean of binary precision and recall.');

const fieldTypes = new Map();
for (const entry of entries) if (entry.field) fieldTypes.set(`${entry.receiver}.${entry.name}`, entry.signature.slice(entry.signature.lastIndexOf(':') + 1).trim());

module.exports = { names, entries, fieldTypes };
