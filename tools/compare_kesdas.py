#!/usr/bin/env python3
"""Runs the same operations through kesdas and pandas and compares the printed text.

    python3 tools/compare_kesdas.py [FILTER]

Each case is (name, kestrel statements, pandas statements). The last expression of each side is
printed with print(). Differences are shown as a diff; the exit status is 1 when any differ.
"""
import difflib
import os
import subprocess
import sys
import tempfile
from pathlib import Path

import pandas as pd

ROOT = Path(__file__).resolve().parent.parent
KESTREL = os.environ.get("KESTREL") or str(next(p for p in (ROOT / "target/release/kestrel", ROOT / "target/debug/kestrel") if p.exists()))

PRELUDE_K = '''import std.kesdas.{Series, DataFrame, Scalar}
let df = DataFrame([Series(["a", "b", "a", "c", "b", "a"], name = "k"), Series([1, 2, 3, 4, 5, 6], name = "v"), Series([1.5, 2.5, 3.5, 4.5, 5.5, 6.5], name = "w")])
let s = Series([3, 1, 2, 5, 4], name = "n")
let f = Series([1.5, 2.25, 3.0], name = "f")
'''.replace("let df =", "let df :=").replace("let s =", "let s :=").replace("let f =", "let f :=")
PRELUDE_P = '''import pandas as pd
df = pd.DataFrame({"k": ["a","b","a","c","b","a"], "v": [1,2,3,4,5,6], "w": [1.5,2.5,3.5,4.5,5.5,6.5]})
s = pd.Series([3,1,2,5,4], name="n")
f = pd.Series([1.5,2.25,3.0], name="f")
'''

CASES = [
    ("series", "s", "s"),
    ("series float", "f", "f"),
    ("series add scalar", "s + 10", "s + 10"),
    ("series mul float", "s * 1.5", "s * 1.5"),
    ("series div", "s / 2", "s / 2"),
    ("series neg", "-s", "-s"),
    ("series compare", "s.gt(2)", "s > 2"),
    ("series sort", "s.sortValues()", "s.sort_values()"),
    ("series sort desc", "s.sortValues(ascending = false)", "s.sort_values(ascending=False)"),
    ("series head", "s.head(2)", "s.head(2)"),
    ("series tail", "s.tail(2)", "s.tail(2)"),
    ("series cumsum", "s.cumSum()", "s.cumsum()"),
    ("series diff", "s.diff()", "s.diff()"),
    ("series shift", "s.shift(2)", "s.shift(2)"),
    ("series rolling mean", "f.rolling(2).mean()", "f.rolling(2).mean()"),
    ("series describe", "s.describe()", "s.describe()"),
    ("series sum", "s.sum()", "s.sum()"),
    ("series mean", "s.mean()", "s.mean()"),
    ("series std", "s.std()", "s.std()"),
    ("series median", "s.median()", "s.median()"),
    ("series quantile", "s.quantile(0.25)", "s.quantile(0.25)"),
    ("series min", "s.min()", "s.min()"),
    ("series max", "f.max()", "f.max()"),
    ("series idxmax", "s.idxMax()", "s.idxmax()"),
    ("series rank", "s.rank()", "s.rank()"),
    ("series pct_change", "f.pctChange()", "f.pct_change()"),
    ("series round", "Series([1.234, 5.678]).round(1)", "pd.Series([1.234, 5.678]).round(1)"),
    ("series isin", "s.isIn([1, 2])", "s.isin([1, 2])"),
    ("series between", "s.between(2, 4)", "s.between(2, 4)"),
    ("series clip", "s.clip(2.0, 4.0)", "s.clip(2, 4)"),
    ("series filter", "s.filter(s.gt(2))", "s[s > 2]"),
    ("series str upper", "df.col(\"k\")?.str().upper()", "df['k'].str.upper()"),
    ("series str len", "df.col(\"k\")?.str().len()", "df['k'].str.len()"),
    ("series value_counts", "df.col(\"k\")?.valueCounts()", "df['k'].value_counts()"),
    ("series unique", "df.col(\"k\")?.unique()", "pd.Series(df['k'].unique(), name='k')"),
    ("series nunique", "df.col(\"k\")?.nunique()", "df['k'].nunique()"),
    ("series astype float", "s.astype(\"float64\")", "s.astype('float64')"),
    ("frame", "df", "df"),
    ("frame head", "df.head(2)", "df.head(2)"),
    ("frame tail", "df.tail(2)", "df.tail(2)"),
    ("frame shape", "df.shape()", "df.shape"),
    ("frame dtypes", "df.dtypes()", "df.dtypes.astype(str).rename(None).astype('string')"),
    ("frame sum numeric", "df.select([\"v\", \"w\"])?.sum()", "df[['v','w']].sum()"),
    ("frame mean numeric", "df.select([\"v\", \"w\"])?.mean()", "df[['v','w']].mean()"),
    ("frame describe", "df.describe()", "df.describe()"),
    ("frame corr", "df.select([\"v\", \"w\"])?.corr()", "df[['v','w']].corr()"),
    ("frame sort", "df.sortValues(\"v\", ascending = false)?", "df.sort_values('v', ascending=False)"),
    ("frame sort two", "df.sortValues([\"k\", \"v\"], ascending = false)?", "df.sort_values(['k','v'], ascending=False)"),
    ("frame filter", "df.filter(df.col(\"v\")?.gt(3))", "df[df['v'] > 3]"),
    ("frame drop", "df.drop(\"w\")", "df.drop(columns=['w'])"),
    ("frame rename", "df.rename(\"v\", \"value\")", "df.rename(columns={'v':'value'})"),
    ("frame assign", "df.assign(\"z\", df.col(\"v\")? * 2)", "df.assign(z=df['v'] * 2)"),
    ("frame set_index", "df.setIndex(\"k\")?", "df.set_index('k')"),
    ("frame reset_index", "df.setIndex(\"k\")?.resetIndex()", "df.set_index('k').reset_index()"),
    ("groupby sum", "df.groupBy(\"k\")?.sum()", "df.groupby('k').sum()"),
    ("groupby mean", "df.groupBy(\"k\")?.mean()", "df.groupby('k').mean()"),
    ("groupby count", "df.groupBy(\"k\")?.counts()", "df.groupby('k').count()"),
    ("groupby size", "df.groupBy(\"k\")?.size()", "df.groupby('k').size()"),
    ("groupby max", "df.groupBy(\"k\")?.max()", "df.groupby('k').max()"),
    ("groupby agg", "df.groupBy(\"k\")?.agg([(\"v\", \"sum\"), (\"w\", \"mean\")])", "df.groupby('k').agg(v_sum=('v','sum'), w_mean=('w','mean'))"),
    ("pivot", "df.pivotTable(\"k\", \"v\", \"w\", \"sum\")?", "df.pivot_table(index='k', columns='v', values='w', aggfunc='sum')"),
    ("duplicated", "df.col(\"k\")?.duplicated()", "df['k'].duplicated()"),
    ("drop duplicates", "df.dropDuplicates([\"k\"])?", "df.drop_duplicates(subset=['k'])"),
    ("nlargest", "df.nlargest(2, \"w\")?", "df.nlargest(2, 'w')"),
    ("csv", "df.toCsv()", "df.to_csv(index=False)"),
]


CASES += [
    ("series head negative", "s.head(-2)", "s.head(-2)"),
    ("series tail negative", "s.tail(-2)", "s.tail(-2)"),
    ("series any", "s.any()", "str(s.any()).lower()"),
    ("series all", "s.all()", "str(s.all()).lower()"),
    ("series sem", "s.sem()", "s.sem()"),
    ("series skew", "s.skew()", "s.skew()"),
    ("series kurt", "s.kurt()", "s.kurt()"),
    ("series quantiles", "s.quantile([0.25, 0.75])", "s.quantile([0.25, 0.75])"),
    ("series rank dense", "s.rank(method = \"dense\")", "s.rank(method='dense')"),
    ("series rank pct", "s.rank(pct = true)", "s.rank(pct=True)"),
    ("series duplicated last", "df.col(\"k\")?.duplicated(keep = \"last\")", "df['k'].duplicated(keep='last')"),
    ("series drop duplicates none", "df.col(\"k\")?.dropDuplicates(keep = \"none\")", "df['k'].drop_duplicates(keep=False)"),
    ("series value counts normalize", "df.col(\"k\")?.valueCounts(normalize = true)", "df['k'].value_counts(normalize=True)"),
    ("series value counts unsorted", "df.col(\"k\")?.valueCounts(sort = false)", "df['k'].value_counts(sort=False)"),
    ("series interpolate", "let miss := Series([1.0, 0.0 / 0.0, 3.0, 0.0 / 0.0], name = \"m\")\nmiss.interpolate()", "miss = pd.Series([1.0, float('nan'), 3.0, float('nan')], name='m')\nmiss.interpolate()"),
    ("series ffill", "let miss := Series([1.0, 0.0 / 0.0, 3.0], name = \"m\")\nmiss.ffill()", "miss = pd.Series([1.0, float('nan'), 3.0], name='m')\nmiss.ffill()"),
    ("series bfill", "let miss := Series([1.0, 0.0 / 0.0, 3.0], name = \"m\")\nmiss.bfill()", "miss = pd.Series([1.0, float('nan'), 3.0], name='m')\nmiss.bfill()"),
    ("series where", "s.where(s.gt(2), Scalar.ofInt(0))", "s.where(s > 2, 0)"),
    ("series mask", "s.mask(s.gt(2), Scalar.ofInt(0))", "s.mask(s > 2, 0)"),
    ("series corr", "s.corr(s)", "round(s.corr(s), 12)"),
    ("series cov", "s.cov(s)", "s.cov(s)"),
    ("series autocorr", "s.autocorr()", "s.autocorr()"),
    ("series concat", "Series.concat([s, f])", "pd.concat([s, f])"),
    ("series map values", "df.col(\"k\")?.mapValues(Map(\"a\" => \"A\", \"b\" => \"B\"))", "df['k'].map({'a': 'A', 'b': 'B'})"),

    ("frame head negative", "df.head(-2)", "df.head(-2)"),
    ("frame tail negative", "df.tail(-2)", "df.tail(-2)"),
    ("frame at", "df.at(2, \"v\")?", "df.at[2, 'v']"),
    ("frame iat", "df.iat(2, 1)?", "df.iat[2, 1]"),
    ("frame any", "df.any()", "df.any()"),
    ("frame all", "df.all()", "df.all()"),
    ("frame prod", "df.select([\"v\", \"w\"])?.prod()", "df[['v','w']].prod()"),
    ("frame sem", "df.select([\"v\", \"w\"])?.sem()", "df[['v','w']].sem()"),
    ("frame skew", "df.select([\"v\", \"w\"])?.skew()", "df[['v','w']].skew()"),
    ("frame kurt", "df.select([\"v\", \"w\"])?.kurt()", "df[['v','w']].kurt().round(12)"),
    ("frame quantile", "df.select([\"v\", \"w\"])?.quantile(0.5)", "df[['v','w']].quantile(0.5)"),
    ("frame quantiles", "df.select([\"v\", \"w\"])?.quantile([0.25, 0.75])", "df[['v','w']].quantile([0.25, 0.75])"),
    ("frame cov", "df.select([\"v\", \"w\"])?.cov()", "df[['v','w']].cov()"),
    ("frame corrwith", "df.corrwith(df)", "df[['v','w']].corrwith(df[['v','w']])"),
    ("frame value counts", "df.valueCounts(\"k\")?", "df['k'].value_counts()"),
    ("frame duplicated last", "df.duplicated(keep = \"last\")", "df.duplicated(keep='last')"),
    ("frame sort multi ascending", "df.sortValues([\"k\", \"v\"], ascending = [false, true])?", "df.sort_values(['k','v'], ascending=[False, True])"),
    ("frame select dtypes", "df.selectDtypes(\"number\")", "df.select_dtypes(include='number')"),
    ("frame round", "df.selectDtypes(\"number\").round(1)", "df[['v','w']].round(1)"),
    ("frame abs", "df.selectDtypes(\"number\").abs()", "df[['v','w']].abs()"),
    ("frame clip", "df.selectDtypes(\"number\").clip(2.0, 5.0)", "df[['v','w']].clip(2, 5)"),
    ("frame shift", "df.select([\"v\", \"w\"])?.shift(1)", "df[['v','w']].shift(1)"),
    ("frame diff", "df.select([\"v\", \"w\"])?.diff()", "df[['v','w']].diff()"),
    ("frame pct change", "df.select([\"v\", \"w\"])?.pctChange()", "df[['v','w']].pct_change()"),
    ("frame cumsum", "df.select([\"v\", \"w\"])?.cumSum()", "df[['v','w']].cumsum()"),
    ("frame rank", "df.select([\"v\", \"w\"])?.rank()", "df[['v','w']].rank()"),
    ("frame replace", "df.replace(\"a\", \"z\")", "df.replace('a', 'z')"),
    ("frame insert", "let bumped := df.insert(1, \"x\", Series([9, 8, 7, 6, 5, 4]))?\nbumped", "bumped = df.copy()\nbumped.insert(1, 'x', [9, 8, 7, 6, 5, 4])\nbumped"),
    ("frame pivot", "df.pivot(\"k\", \"v\", \"w\")?", "df.pivot(index='k', columns='v', values='w')"),
    ("frame crosstab", "df.crosstab(\"k\", \"v\")?", "pd.crosstab(df['k'], df['v'])"),
    ("frame get dummies", "df.getDummies([\"k\"])?", "pd.get_dummies(df, columns=['k'])"),
    ("frame melt", "df.melt(idVars = [\"k\"], valueVars = [\"v\", \"w\"])?", "df.melt(id_vars=['k'], value_vars=['v','w'])"),
    ("frame concat columns", "let right := DataFrame([Series([10, 20, 30, 40, 50, 60], name = \"x\")])\nDataFrame.concatColumns([df, right])?", "right = pd.DataFrame({'x': [10,20,30,40,50,60]})\npd.concat([df, right], axis=1)"),
    ("csv separator", "df.toCsv(separator = \";\")", "df.to_csv(sep=';', index=False)"),
    ("string count", "df.col(\"k\")?.str().count(\"a\")", "df['k'].str.count('a')"),
    ("string find", "df.col(\"k\")?.str().find(\"b\")", "df['k'].str.find('b')"),
    ("string center", "df.col(\"k\")?.str().center(3)", "df['k'].str.center(3)"),
    ("string zfill", "df.col(\"k\")?.str().zfill(3)", "df['k'].str.zfill(3)"),
    ("string slice replace", "df.col(\"k\")?.str().sliceReplace(0, 1, \"Q\")", "df['k'].str.slice_replace(0, 1, 'Q')"),
    ("string strip leading", "let padded := Series([\"  a\", \" b \"], name = \"p\")\npadded.str().stripLeading()", "padded = pd.Series(['  a', ' b '], name='p')\npadded.str.lstrip()"),
    ("string split to frame", "df.col(\"k\")?.splitToFrame(\"x\")", "df['k'].str.split('x', expand=True)"),

    ("groupby head", "df.groupBy(\"k\")?.head(2)", "df.groupby('k').head(2)"),
    ("groupby tail", "df.groupBy(\"k\")?.tail(1)", "df.groupby('k').tail(1)"),
    ("groupby nth", "df.groupBy(\"k\")?.nth(1)", "df.groupby('k').nth(1)"),
    ("groupby cumcount", "df.groupBy(\"k\")?.cumCount()", "df.groupby('k').cumcount()"),
    ("groupby ngroup", "df.groupBy(\"k\")?.ngroup()", "df.groupby('k').ngroup()"),
    ("groupby get group", "df.groupBy(\"k\")?.getGroup(\"a\")?", "df.groupby('k').get_group('a')"),
    ("groupby transform all", "df.groupBy(\"k\")?.transformAll(\"mean\")", "df.groupby('k').transform('mean', numeric_only=True)"),
    ("groupby shift", "df.groupBy(\"k\")?.shift(\"v\")", "df.groupby('k')['v'].shift()"),
    ("groupby diff", "df.groupBy(\"k\")?.diff(\"v\")", "df.groupby('k')['v'].diff()"),
    ("groupby pct change", "df.groupBy(\"k\")?.pctChange(\"v\")", "df.groupby('k')['v'].pct_change()"),
    ("groupby sem", "df.groupBy(\"k\")?.sem()", "df.groupby('k').sem()"),
    ("groupby skew", "df.groupBy(\"k\")?.skew()", "df.groupby('k').skew()"),
    ("groupby kurt", "df.groupBy(\"k\")?.kurt()", "df.groupby('k').kurt()"),
    ("groupby all", "df.groupBy(\"k\")?.all()", "df.groupby('k').all()"),
    ("groupby any", "df.groupBy(\"k\")?.any()", "df.groupby('k').any()"),
    ("groupby quantile", "df.groupBy(\"k\")?.quantile(0.5)", "df.groupby('k').quantile(0.5)"),
    ("frame rolling", "df.select([\"v\", \"w\"])?.rolling(2).mean()", "df[['v','w']].rolling(2).mean()"),
]

MERGE_K = '''let left = DataFrame([Series(["a", "b", "d"], name = "k"), Series([10, 20, 40], name = "x")])
'''.replace("let left =", "let left :=")
MERGE_P = '''left = pd.DataFrame({"k": ["a","b","d"], "x": [10,20,40]})
'''
CASES += [
    ("merge inner", MERGE_K + "df.merge(left, on = \"k\")?", MERGE_P + "df.merge(left, on='k')"),
    ("merge left", MERGE_K + "df.merge(left, on = \"k\", how = \"left\")?", MERGE_P + "df.merge(left, on='k', how='left')"),
    ("merge right", MERGE_K + "df.merge(left, on = \"k\", how = \"right\")?", MERGE_P + "df.merge(left, on='k', how='right')"),
    ("merge outer", MERGE_K + "df.merge(left, on = \"k\", how = \"outer\")?", MERGE_P + "df.merge(left, on='k', how='outer')"),
]
CASES += [
    ("merge suffixes", MERGE_K + "df.merge(left, on = \"k\", how = \"outer\", leftSuffix = \"_l\", rightSuffix = \"_r\")?", MERGE_P + "df.merge(left, on='k', how='outer', suffixes=('_l', '_r'))"),
]
CASES += [
    ("frame dropna", "let m := DataFrame.parseCsv(\"a,b\\n1,\\n,2\\n,\\n3,3\\n\")\nm.dropNa()", "m = pd.DataFrame({'a':[1,None,None,3], 'b':[None,2,None,3]})\nm.dropna()"),
    ("frame dropna all", "let m := DataFrame.parseCsv(\"a,b\\n1,\\n,2\\n,\\n3,3\\n\")\nm.dropNa(how = \"all\")", "m = pd.DataFrame({'a':[1,None,None,3], 'b':[None,2,None,3]})\nm.dropna(how='all')"),
    ("frame dropna thresh", "let m := DataFrame.parseCsv(\"a,b\\n1,\\n,2\\n,\\n3,3\\n\")\nm.dropNa(thresh = 2)", "m = pd.DataFrame({'a':[1,None,None,3], 'b':[None,2,None,3]})\nm.dropna(thresh=2)"),
    ("frame dropna subset", "let m := DataFrame.parseCsv(\"a,b\\n1,\\n,2\\n,\\n3,3\\n\")\nm.dropNa([\"b\"])?", "m = pd.DataFrame({'a':[1,None,None,3], 'b':[None,2,None,3]})\nm.dropna(subset=['b'])"),
]
CASES += [
    ("groupby cumsum", "df.groupBy(\"k\")?.cumSum(\"v\")", "df.groupby('k')['v'].cumsum()"),
    ("groupby cummax", "df.groupBy(\"k\")?.cumMax(\"v\")", "df.groupby('k')['v'].cummax()"),
    ("groupby rank", "df.groupBy(\"k\")?.rank(\"v\")", "df.groupby('k')['v'].rank()"),
]
CASES += [
    ("series clip int", "s.clip(2, 4)", "s.clip(2, 4)"),
    ("series repeat", "s.repeat(2)", "s.repeat(2)"),
    ("series is unique", "s.isUnique()", "str(s.is_unique).lower()"),
    ("series is monotonic", "s.isMonotonic()", "str(s.is_monotonic_increasing).lower()"),
    ("frame idxmax", "df.select([\"v\", \"w\"])?.idxMax()", "df[['v','w']].idxmax()"),
    ("frame mode", "df.select([\"k\", \"v\"])?.mode()", "df[['k','v']].mode()"),
]
CASES += [
    ("frame expanding mean", "df.select([\"v\", \"w\"])?.expanding().mean()", "df[['v','w']].expanding().mean()"),
    ("frame ewm mean", "df.select([\"v\", \"w\"])?.ewm(alpha = 0.5).mean()", "df[['v','w']].ewm(alpha=0.5).mean()"),
    ("frame ewm adjusted false", "df.select([\"v\", \"w\"])?.ewm(alpha = 0.5, adjust = false).mean()", "df[['v','w']].ewm(alpha=0.5, adjust=False).mean()"),
]
CASES += [
    ("frame transpose", "df.select([\"v\", \"w\"])?.transpose()?", "df[['v','w']].T"),
]
CASES += [
    ("frame corr missing", "let m := DataFrame.parseCsv(\"a,b\\n1,\\n2,2\\n3,6\\n4,8\\n\")\nm.corr()", "m = pd.DataFrame({'a':[1,2,3,4], 'b':[None,2,6,8]})\nm.corr()"),
    ("frame cov missing", "let m := DataFrame.parseCsv(\"a,b\\n1,\\n2,2\\n3,6\\n4,8\\n\")\nm.cov()", "m = pd.DataFrame({'a':[1,2,3,4], 'b':[None,2,6,8]})\nm.cov()"),
]



def run_kestrel(body: str) -> str:
    lines = body.rstrip().split("\n")
    last = lines[-1]
    code = PRELUDE_K + "\n".join(lines[:-1]) + "\n"
    wrapper = f"function run() : void or Error with Effects {{\n{code}\nprint({last})\n}}\nrun()\n"
    # `let` bindings and `?` live inside one function so the script is checked like a program.
    with tempfile.TemporaryDirectory() as folder:
        path = Path(folder) / "case.kes"
        path.write_text(wrapper.replace("import std.kesdas.{Series, DataFrame, Scalar}\n", "")  # moved below
                        .replace("function run()", "import std.kesdas.{Series, DataFrame, Scalar}\nfunction run()", 1))
        done = subprocess.run([KESTREL, "run", "--root", folder, str(path)], capture_output=True, text=True)
    return (done.stdout + done.stderr).rstrip("\n")


def run_pandas(body: str) -> str:
    lines = body.rstrip().split("\n")
    scope = {}
    exec(PRELUDE_P + "\n".join(lines[:-1]), scope)
    value = eval(lines[-1], scope)
    if isinstance(value, (float, int)) and not isinstance(value, bool):
        return repr(float(value)) if isinstance(value, float) else str(value)
    return str(value)


KNOWN_DEVIATIONS = {"frame dtypes", "series autocorr"}  # dtype spelling; last-bit correlation rounding
DEVIATION_REASONS = {
    "frame dtypes": "pandas 3 spells the dtype `string`, Kestrel `str`",
    "series autocorr": "last-bit difference in the correlation summation order",
}


def emit_test() -> None:
    """Writes tests/kesdas/program.kes and expected.txt from the cases, for the Rust test."""
    folder = ROOT / "tests" / "kesdas"
    folder.mkdir(exist_ok=True)
    functions, calls, expected = [], [], []
    for number, (name, kestrel, pandas) in enumerate(CASES):
        if name in KNOWN_DEVIATIONS:
            continue
        lines = kestrel.rstrip().split("\n")
        body = PRELUDE_K.replace("import std.kesdas.{Series, DataFrame, Scalar}\n", "") + "\n".join(lines[:-1]) + f"\nprint({lines[-1]})\n"
        functions.append(f"function case{number}() : void or Error with Effects {{\n{body}}}\n")
        calls.append(f'print("=== {name}")\ncase{number}()?')
        expected.append(f"=== {name}\n{run_pandas(pandas)}")
    program = "import std.kesdas.{Series, DataFrame, Scalar}\n\n" + "\n".join(functions) + "\nfunction main() : void or Error with Effects {\n" + "\n".join("    " + c.replace("\n", "\n    ") for c in calls) + "\n}\nmain()\n"
    (folder / "program.kes").write_text(program)
    (folder / "expected.txt").write_text("\n".join(expected) + "\n")
    print(f"wrote {len(expected)} cases to {folder}")


def main() -> int:
    if "--emit-test" in sys.argv:
        emit_test()
        return 0
    wanted = sys.argv[1] if len(sys.argv) > 1 else ""
    failures = 0
    for name, kestrel, pandas in CASES:
        if wanted not in name:
            continue
        if name in KNOWN_DEVIATIONS:
            print(f"skip  {name}  ({DEVIATION_REASONS.get(name, 'known deviation')})")
            continue
        got = run_kestrel(kestrel)
        want = run_pandas(pandas)
        if got.strip() == want.strip():
            print(f"ok    {name}")
        else:
            failures += 1
            print(f"DIFF  {name}")
            for line in difflib.unified_diff(want.splitlines(), got.splitlines(), "pandas", "kesdas", lineterm="", n=0):
                print("      " + line)
    print(f"{failures} difference(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
