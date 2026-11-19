"""Tests for the notebook's conservative type reader, without starting a kernel."""

import unittest

from kestrel_kernel.analysis import Analyzer


class AnalysisTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.analyzer = Analyzer()

    def test_cell_bindings_keep_their_types(self):
        source = (
            "import std.kesdas.{Series, DataFrame}\n"
            'let df := DataFrame([Series([1, 2], name = "v")])\n'
            'let column := df.col("v")?\n'
            "let total := column.sum()\n"
            "let value := total.asDouble()\n"
        )
        env = self.analyzer.env_after(source, {})
        self.assertEqual(env["df"], "DataFrame")
        self.assertEqual(env["column"], "Series")
        self.assertEqual(env["total"], "Scalar")
        self.assertEqual(env["value"], "double")

    def test_member_receiver_keeps_the_question_mark(self):
        source = 'df.col("v")?.'
        receiver, prefix = self.analyzer.member_receiver(
            'import std.kesdas.{Series, DataFrame}\nlet df := DataFrame([Series([1], name = "v")])\n' + source,
            len('import std.kesdas.{Series, DataFrame}\nlet df := DataFrame([Series([1], name = "v")])\n') + len(source),
        )
        self.assertEqual(receiver, 'df.col("v")?')
        self.assertEqual(prefix, "")

    def test_common_chains_have_types(self):
        env = {"df": "DataFrame", "s": "Series", "g": "GroupBy", "scalar": "Scalar"}
        cases = {
            'df.col("v")': "Series or Error",
            'df.col("v")?': "Series",
            'df.groupBy("v")?': "GroupBy",
            "df.shape()": "(Int, Int)",
            "df.names": "List[String]",
            'df.sortValues("v")?': "DataFrame",
            "df.describe()": "DataFrame",
            "DataFrame.empty()": "DataFrame",
            'DataFrame.parseCsv("a\\n1\\n")': "DataFrame",
            'readCsv("data.csv")?': "DataFrame",
            "s.mean()": "Scalar",
            "s.str().upper()": "Series",
            "s.rolling(2).sum()": "Series",
            "s * 2.0": "Series",
            "s.iloc(0)": "Scalar",
            "s.iloc([0, 1])": "Series",
            "scalar.asDouble()": "double",
            "df.names": "List[String]",
            "df.names.toList()": "List[String]",
            "df.toCsv()": "String",
            'df.col("v")?.name': "String",
            "df.col(\"v\")?.name.toUpper()": "String",
            "df.dropNa()": "DataFrame",
            'df.dropNa(["v"])?': "DataFrame",
            'df.dropDuplicates(["v"])?': "DataFrame",
            'df.col("v").sum()': "",
            'df.col("v")?.sum()': "Scalar",
        }
        for expression, expected in cases.items():
            with self.subTest(expression=expression):
                self.assertEqual(self.analyzer.infer(expression, env), expected)

    def test_member_candidates_follow_the_receiver(self):
        env = {"df": "DataFrame", "s": "Series"}
        frame = {entry["name"] for entry in self.analyzer.member_candidates("df", env)}
        self.assertIn("col", frame)
        self.assertIn("groupBy", frame)
        self.assertIn("names", frame)
        self.assertNotIn("upper", frame)
        series = {entry["name"] for entry in self.analyzer.member_candidates("s", env)}
        self.assertIn("mean", series)
        self.assertIn("str", series)
        self.assertNotIn("groupBy", series)
        self.assertEqual(self.analyzer.member_candidates('df.col("v")', env), [])
        names = {entry["name"] for entry in self.analyzer.member_candidates("df.names", env)}
        self.assertIn("toList", names)
        self.assertIn("length", names)
        text = {entry["name"] for entry in self.analyzer.member_candidates("df.toCsv()", env)}
        self.assertIn("toUpper", text)

    def test_namespace_imports_bind_types_and_functions(self):
        source = (
            "import std.kesdas as kd\n"
            'let df := kd.DataFrame([kd.Series([1, 2], name = "v")])\n'
            "let column := df.col(\"v\")?\n"
            'let loaded := kd.readCsv("data.csv")?\n'
        )
        env = self.analyzer.env_after(source, {})
        self.assertEqual(env["kd"], "@module:std.kesdas")
        self.assertEqual(env["df"], "DataFrame")
        self.assertEqual(env["column"], "Series")
        self.assertEqual(env["loaded"], "DataFrame")
        self.assertEqual(
            self.analyzer.infer("kd.DataFrame.empty()", {"kd": "@module:std.kesdas"}),
            "DataFrame",
        )
        names = {entry["name"] for entry in self.analyzer.member_candidates("kd", env)}
        self.assertIn("DataFrame", names)
        self.assertIn("readCsv", names)
        statics = {entry["name"] for entry in self.analyzer.member_candidates("kd.DataFrame", env)}
        self.assertIn("empty", statics)
        self.assertNotIn("col", statics)

    def test_list_members_include_at_and_keep_the_item_type(self):
        env = {"names": "List[String]", "numbers": "List[Int]"}
        self.assertIn("at", {entry["name"] for entry in self.analyzer.member_candidates("names", env)})
        self.assertEqual(self.analyzer.infer("names.at(0)", env), "String")
        self.assertEqual(self.analyzer.infer("numbers.at(2).abs()", env), "Int")
        self.assertIn("at", self.analyzer.member_signature("List[Int]", "at"))

    def test_unknown_receivers_offer_nothing(self):
        self.assertEqual(self.analyzer.member_candidates("mystery", {}), [])

    def test_tensor_constructors_show_literal_shapes(self):
        cases = {
            "let t := Tensor([0, 0])": "Tensor[double, (2,)]",
            "let t := Tensor([0, 0], shape = (1, 2))": "Tensor[double, (1, 2)]",
            "let t := Tensor(Array([0, 0]))": "DynTensor[double]",
            "let t := Tensor.zeros((2, 3))": "Tensor[double, (2, 3)]",
            "let t := Tensor.ones((2, 3))": "Tensor[double, (2, 3)]",
        }
        for source, expected in cases.items():
            with self.subTest(source=source):
                self.assertEqual(self.analyzer.env_after(source, {})["t"], expected)
        self.assertNotIn("length", self.analyzer.env_after("let t := Tensor([0, 0])", {})["t"])

    def test_numerical_namespace_imports_complete_and_type(self):
        source = (
            "import std.linalg\n"
            "import std.math\n"
            "import std.ml\n"
            "import std.random\n"
        )
        env = self.analyzer.env_after(source, {})
        for alias, expected in {
            "linalg": {"inverse", "svd", "det"},
            "math": {"exp", "log", "sin"},
            "ml": {"StandardScaler", "trainTestSplit", "meanSquaredError"},
            "random": {"Rng", "seed"},
        }.items():
            names = {entry["name"] for entry in self.analyzer.member_candidates(alias, env)}
            self.assertTrue(expected <= names, f"{alias} exports {sorted(expected - names)}")

        self.assertEqual(self.analyzer.infer("math.exp(1.0)", env), "double")
        self.assertEqual(self.analyzer.infer("random.Rng.seeded(1)", env), "Rng")
        self.assertEqual(
            self.analyzer.infer("ml.StandardScaler.fit(Tensor.ones((2, 2)))", env),
            "FittedStandardScaler",
        )
        self.assertEqual(
            self.analyzer.member_candidates("random.Rng", env)[0]["name"], "seeded"
        )
        inverse = self.analyzer.infer("linalg.inverse(Tensor.ones((2, 2)))", env)
        self.assertEqual(inverse, "DynTensor[double]")

    def test_conversion_pipes_keep_their_result_types(self):
        self.assertEqual(self.analyzer.infer("1 |> double", {}), "double")
        self.assertEqual(self.analyzer.infer("[0, 0] |> Tensor", {}), "Tensor[double, (2,)]")


    def test_kesnet_variables_and_layers_have_types_and_members(self):
        source = (
            "import std.kesnet.{Linear, Sequential, Module}\n"
            "let x := Variable.zeros((2, 3))\n"
            "let layer := Linear(3, 4)\n"
            "let y := layer.forward(x)\n"
            "let z := y.relu()\n"
            "let total := z.sum()\n"
            "let shape := total.shape()\n"
            "let gradient := total.grad()\n"
            "let data := total.data()\n"
        )
        env = self.analyzer.env_after(source, {})
        self.assertEqual(env["x"], "Variable")
        self.assertEqual(env["layer"], "Linear")
        self.assertEqual(env["y"], "Variable")
        self.assertEqual(env["z"], "Variable")
        self.assertEqual(env["total"], "Variable")
        self.assertEqual(env["shape"], "Array[Int]")
        self.assertEqual(env["gradient"], "Option[DynTensor[double]]")
        self.assertEqual(env["data"], "DynTensor[double]")
        members = {entry["name"] for entry in self.analyzer.member_candidates("x", env)}
        for name in ("relu", "matmul", "softmax", "backward", "shape", "conv2d"):
            self.assertIn(name, members)
        layer_members = {entry["name"] for entry in self.analyzer.member_candidates("layer", env)}
        for name in ("forward", "weight", "parameters", "zeroGrad"):
            self.assertIn(name, layer_members)
        self.assertEqual(self.analyzer.infer("x * 2.0 + x", env), "Variable")
        self.assertEqual(self.analyzer.infer("Variable.zerosLike(x)", env), "Variable")
        self.assertEqual(self.analyzer.infer("Variable.liveNodes()", env), "Int")
        self.assertEqual(self.analyzer.infer("layer.parameters()", env), "List[Variable]")
        self.assertEqual(self.analyzer.infer("layer.parameterCount()", env), "Int")
        self.assertIn("Linear", self.analyzer.module_names("std.kesnet"))
        self.assertIn("Adam", self.analyzer.module_names("std.kesnet.optim"))


if __name__ == "__main__":
    unittest.main()
