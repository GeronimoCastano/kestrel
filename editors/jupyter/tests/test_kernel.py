"""End-to-end tests: start the kernel through jupyter_client and run cells.

Run from this folder with:  python3 -m unittest discover tests
They need a built kestrel (target/debug or target/release), or KESTREL set.
"""

import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from queue import Empty

from jupyter_client.manager import KernelManager

HERE = Path(__file__).resolve().parent
PACKAGE_ROOT = HERE.parent


class KernelTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.folder = tempfile.TemporaryDirectory()
        spec = Path(cls.folder.name) / "kernels" / "kestrel"
        spec.mkdir(parents=True)
        (spec / "kernel.json").write_text(
            json.dumps(
                {
                    "argv": [sys.executable, "-m", "kestrel_kernel", "-f", "{connection_file}"],
                    "display_name": "Kestrel",
                    "language": "kestrel",
                }
            )
        )
        os.environ["JUPYTER_PATH"] = cls.folder.name
        env = dict(os.environ)
        env["PYTHONPATH"] = str(PACKAGE_ROOT) + os.pathsep + env.get("PYTHONPATH", "")
        # A relative KESTREL is relative to this checkout, not the notebook's working folder.
        if env.get("KESTREL") and not Path(env["KESTREL"]).is_absolute():
            env["KESTREL"] = str((PACKAGE_ROOT / env["KESTREL"]).resolve())
        cls.manager = KernelManager(kernel_name="kestrel")
        cls.manager.start_kernel(env=env, cwd=cls.folder.name)
        cls.client = cls.manager.client()
        cls.client.start_channels()
        cls.client.wait_for_ready(timeout=60)

    @classmethod
    def tearDownClass(cls):
        cls.client.stop_channels()
        cls.manager.shutdown_kernel(now=True)
        cls.folder.cleanup()

    def run_cell(self, code, timeout=180):
        """Returns (reply, outputs) where outputs are the iopub messages for the cell."""
        message_id = self.client.execute(code)
        outputs = []
        while True:
            message = self.client.get_iopub_msg(timeout=timeout)
            if message["parent_header"].get("msg_id") != message_id:
                continue
            kind = message["msg_type"]
            if kind == "status" and message["content"]["execution_state"] == "idle":
                break
            if kind in ("stream", "execute_result", "error", "display_data"):
                outputs.append(message)
        reply = self.client.get_shell_msg(timeout=timeout)
        return reply["content"], outputs

    def texts(self, outputs):
        return [
            m["content"]["data"]["text/plain"] for m in outputs if m["msg_type"] == "execute_result"
        ]

    def test_1_last_expression_is_displayed(self):
        reply, outputs = self.run_cell("let x := 40\nx + 2")
        self.assertEqual(reply["status"], "ok")
        self.assertEqual(self.texts(outputs), ["42"])

    def test_2_names_stay_between_cells(self):
        self.run_cell("let kept := 7")
        reply, outputs = self.run_cell("kept * 6")
        self.assertEqual(self.texts(outputs), ["42"])

    def test_3_only_the_last_line_displays(self):
        reply, outputs = self.run_cell("1 + 1\n2 + 2\n3 + 3")
        self.assertEqual(self.texts(outputs), ["6"])

    def test_4_a_binding_displays_nothing(self):
        reply, outputs = self.run_cell("let quiet := 5")
        self.assertEqual(self.texts(outputs), [])

    def test_5_print_goes_to_the_stream(self):
        reply, outputs = self.run_cell('print("hello")\n"done"')
        streams = [m["content"]["text"] for m in outputs if m["msg_type"] == "stream"]
        self.assertEqual("".join(streams), "hello\n")
        self.assertEqual(self.texts(outputs), ["done"])

    def test_6_compile_errors_are_reported(self):
        reply, outputs = self.run_cell("let broken := missingName")
        self.assertEqual(reply["status"], "error")
        self.assertTrue(any("E_UNKNOWN_NAME" in m["content"]["evalue"] + "".join(m["content"]["traceback"]) for m in outputs if m["msg_type"] == "error"))

    def test_7_earlier_statements_survive_a_later_error(self):
        reply, _ = self.run_cell("let first := 1\nlet second := nope")
        self.assertEqual(reply["status"], "error")
        _, outputs = self.run_cell("first")
        self.assertEqual(self.texts(outputs), ["1"])

    def test_8_series_shows_text_and_html(self):
        self.run_cell("import std.kesdas.{Series, DataFrame}")
        reply, outputs = self.run_cell('Series([3, 1, 2], name = "n")')
        self.assertEqual(reply["status"], "ok")
        data = [m["content"]["data"] for m in outputs if m["msg_type"] == "execute_result"][0]
        self.assertIn("Name: n, dtype: int64", data["text/plain"])
        self.assertIn("<table", data["text/html"])

    def test_9_dataframe_chain_with_question_mark(self):
        self.run_cell("import std.kesdas.{Series, DataFrame}")
        self.run_cell('let df := DataFrame([Series(["a", "b", "a"], name = "k"), Series([1, 2, 3], name = "v")])')
        reply, outputs = self.run_cell('df.groupBy("k")?.sum()')
        self.assertEqual(reply["status"], "ok")
        text = self.texts(outputs)[0]
        self.assertIn("a  4", text)

    def test_10_a_missing_column_is_an_error_not_a_crash(self):
        self.run_cell("import std.kesdas.{Series, DataFrame}")
        self.run_cell('let frame := DataFrame([Series([1, 2], name = "v")])')
        reply, outputs = self.run_cell('frame.col("nope")?.mean()\nlet unreachable := 1')
        errors = [m for m in outputs if m["msg_type"] == "error"]
        self.assertTrue(errors)
        self.assertIn("MissingColumn", errors[0]["content"]["evalue"])
        # The session is still alive and the earlier frame is still there.
        _, outputs = self.run_cell("frame.length()")
        self.assertEqual(self.texts(outputs), ["2"])

    def test_11_completeness_check(self):
        message_id = self.client.is_complete("let a := (1 +")
        reply = self.client.get_shell_msg(timeout=60)
        self.assertEqual(reply["content"]["status"], "incomplete")

    def test_12_completion_offers_names(self):
        self.run_cell("let temperature := 3")
        message_id = self.client.complete("temp", 4)
        reply = self.client.get_shell_msg(timeout=60)
        self.assertIn("temperature", reply["content"]["matches"])

    def completion(self, code, cursor=None):
        cursor = len(code) if cursor is None else cursor
        self.client.complete(code, cursor)
        return self.client.get_shell_msg(timeout=60)["content"]["matches"]

    def test_13_dataframe_members_are_typed(self):
        self.run_cell("import std.kesdas.{Series, DataFrame}")
        self.run_cell('let df := DataFrame([Series([1, 2], name = "v")])')
        names = self.completion("df.")
        for name in ("col", "groupBy", "sortValues", "assign", "merge", "names"):
            self.assertIn(name, names)
        self.assertNotIn("upper", names)
        self.assertNotIn("push", names)

    def test_14_column_and_scalar_chains_are_typed(self):
        self.run_cell("import std.kesdas.{Series, DataFrame}")
        self.run_cell('let df := DataFrame([Series([1, 2], name = "v")])')
        self.assertEqual(self.completion('df.col("v").'), [])
        column = self.completion('df.col("v")?.')
        for name in ("mean", "sum", "str", "rolling", "describe"):
            self.assertIn(name, column)
        self.assertNotIn("groupBy", column)
        scalar = self.completion('df.col("v")?.mean().')
        for name in ("asDouble", "asInt", "isMissing", "text"):
            self.assertIn(name, scalar)

    def test_15_a_binding_in_the_current_cell_is_visible(self):
        code = (
            "import std.kesdas.{Series, DataFrame}\n"
            'let fresh := DataFrame([Series([1], name = "v")])\n'
            "fresh."
        )
        names = self.completion(code)
        self.assertIn("col", names)
        self.assertIn("groupBy", names)

    def test_17_namespace_imports_complete_and_type(self):
        self.run_cell("import std.kesdas as kd")
        names = self.completion("kd.")
        for name in ("DataFrame", "Series", "readCsv"):
            self.assertIn(name, names)
        self.run_cell('let frame := kd.DataFrame([kd.Series([1, 2], name = "v")])')
        members = self.completion("frame.")
        self.assertIn("col", members)
        self.assertIn("groupBy", members)
        statics = self.completion("kd.DataFrame.")
        self.assertIn("empty", statics)
        self.assertNotIn("col", statics)

    def test_19_standard_library_members_are_typed(self):
        self.run_cell("import std.kesdas.{Series, DataFrame}")
        self.run_cell('let df := DataFrame([Series([1, 2], name = "v")])')
        names = self.completion("df.names.")
        self.assertIn("toList", names)
        self.assertIn("length", names)
        text = self.completion("df.toCsv().")
        self.assertIn("toUpper", text)
        field = self.completion('df.col("v")?.name.')
        self.assertIn("toUpper", field)

    def test_18_a_type_name_offers_statics_only(self):
        self.run_cell("import std.kesdas.{Series, DataFrame}")
        names = self.completion("DataFrame.")
        self.assertIn("parseCsv", names)
        self.assertIn("empty", names)
        self.assertNotIn("col", names)
        self.assertNotIn("names", names)

    def test_20_numerical_modules_complete_their_names(self):
        self.run_cell("import std.linalg\nimport std.math\nimport std.ml\nimport std.random")
        for alias, expected in {
            "linalg": "inverse",
            "math": "exp",
            "ml": "trainTestSplit",
            "random": "Rng",
        }.items():
            names = self.completion(f"{alias}.")
            self.assertIn(expected, names, f"{alias}. offers {expected}: {names[:10]}")
        self.assertIn("inverse", self.completion("linalg.in"))

    def test_16_inspect_shows_a_member_signature(self):
        self.run_cell("import std.kesdas.{Series, DataFrame}")
        self.run_cell('let df := DataFrame([Series([1, 2], name = "v")])')
        code = 'df.col("v")?.mean()'
        self.client.inspect(code, code.index("mean") + 2)
        reply = self.client.get_shell_msg(timeout=60)["content"]
        self.assertTrue(reply["found"])
        self.assertIn("Scalar", reply["data"]["text/plain"])

    def test_21_list_members_include_at(self):
        self.run_cell("let numbers : List[Int] := [10, 20, 30]")
        names = self.completion("numbers.")
        self.assertIn("at", names)
        reply, outputs = self.run_cell("numbers.at(1)")
        self.assertEqual(self.texts(outputs), ["20"])

    def test_22_a_runtime_stop_keeps_the_session(self):
        self.run_cell("let values : List[Int] := [10, 20, 30]")
        reply, outputs = self.run_cell("values.at(3)")
        self.assertEqual(reply["status"], "error")
        errors = [m for m in outputs if m["msg_type"] == "error"]
        self.assertTrue(errors)
        traceback = "".join(errors[0]["content"]["traceback"])
        self.assertIn("List index out of bounds", traceback)
        self.assertIn("<cell>:1:1", traceback)
        # The list and its names survive the failed cell.
        _, outputs = self.run_cell("values.contains(20)")
        self.assertEqual(self.texts(outputs), ["true"])

    def test_23_error_outputs_are_coloured(self):
        reply, outputs = self.run_cell("missingName")
        self.assertEqual(reply["status"], "error")
        error = [m for m in outputs if m["msg_type"] == "error"][0]["content"]
        # The plain text stays available, and the traceback that clients render is coloured.
        self.assertNotIn("\x1b[", error["evalue"])
        self.assertIn("\x1b[", "".join(error["traceback"]))

if __name__ == "__main__":
    unittest.main()
