"""A small, conservative Kestrel type reader for notebook completion.

The compiler remains the authority. This module only recognises expressions
whose type is obvious from the source and the std.kesdas metadata: names bound
earlier in the notebook, constructors, fields and method results. An expression
it cannot read gets no type, which means completion falls back to a word list
instead of guessing.
"""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Dict, List, Optional, Tuple

IDENTIFIER = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
NUMBER = re.compile(r"^[+-]?(?:0[xX][0-9a-fA-F_]+|\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d[\d_]*)?)$")
BINDING = re.compile(
    r"\b(let|var)\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*(?::\s*([^=;\n]+?))?\s*(?::=|=)"
)
NAMED_ARGUMENT = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*\s*=(?![=>])\s*")
COMMENT = "//"
NUMERIC = {"Int", "double", "BigInt"}
ITERABLE = {"List", "Set", "SortedSet", "MutSet", "Array", "Lazy"}
SERIES_OPS = {"+", "-", "*", "/", "%", "**"}
OPERATORS = ("|>", "||", "&&", "==", "!=", "<=", ">=", "**", "=>", "->", "<", ">", "+", "-", "*", "/", "%", "@")


def skip_string(text: str, start: int) -> int:
    """The index just past the string that starts at `start`."""
    i = start + 1
    while i < len(text):
        if text[i] == "\\":
            i += 2
            continue
        if text[i] == '"':
            return i + 1
        i += 1
    return len(text)


def strip_comments(text: str) -> str:
    """Source with comments blanked, keeping line breaks and string literals."""
    out: List[str] = []
    i = 0
    while i < len(text):
        if text[i] == '"':
            end = skip_string(text, i)
            out.append(text[i:end])
            i = end
            continue
        if text.startswith(COMMENT, i):
            while i < len(text) and text[i] != "\n":
                out.append(" ")
                i += 1
            continue
        out.append(text[i])
        i += 1
    return "".join(out)


def split_top_level(text: str, separator: str = ",") -> List[str]:
    """Split on a separator that is outside brackets and strings."""
    parts: List[str] = []
    start = 0
    round_depth = square = curly = 0
    i = 0
    while i < len(text):
        current = text[i]
        if current == '"':
            i = skip_string(text, i)
            continue
        if current == "(":
            round_depth += 1
        elif current == ")":
            round_depth -= 1
        elif current == "[":
            square += 1
        elif current == "]":
            square -= 1
        elif current == "{":
            curly += 1
        elif current == "}":
            curly -= 1
        elif current == separator and round_depth == square == curly == 0:
            parts.append(text[start:i])
            start = i + 1
        i += 1
    if text[start:].strip():
        parts.append(text[start:])
    return parts


def opener_of(text: str, close_index: int) -> int:
    """The bracket that opens the one closing at `close_index`, or -1."""
    stack: List[int] = []
    i = 0
    while i < len(text):
        current = text[i]
        if current == '"':
            i = skip_string(text, i)
            continue
        if current in "([{":
            stack.append(i)
        elif current in ")]}":
            opened = stack.pop() if stack else -1
            if i == close_index:
                return opened
        i += 1
    return -1


def closer_of(text: str, open_index: int) -> int:
    """The bracket that closes the one opening at `open_index`, or -1."""
    depth = 0
    i = open_index
    while i < len(text):
        current = text[i]
        if current == '"':
            i = skip_string(text, i)
            continue
        if current in "([{":
            depth += 1
        elif current in ")]}":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return -1


def balanced(text: str) -> bool:
    depth = 0
    i = 0
    while i < len(text):
        current = text[i]
        if current == '"':
            i = skip_string(text, i)
            continue
        if current in "([{":
            depth += 1
        elif current in ")]}":
            depth -= 1
            if depth < 0:
                return False
        i += 1
    return depth == 0


def remove_outer_parens(value: str) -> str:
    while value.startswith("(") and value.endswith(")") and balanced(value[1:-1]):
        if len(split_top_level(value[1:-1])) != 1:
            break
        value = value[1:-1].strip()
    return value


def base_type(type_text: str) -> str:
    """`Series or Error` and `Option[Series]` name Series as their base type."""
    match = re.match(r"^(?:&|StoredView\[|Box\[)?(?:dyn\s+)?(?:[A-Za-z_][A-Za-z0-9_]*\.)*([A-Za-z_][A-Za-z0-9_]*)", (type_text or "").strip())
    return match.group(1) if match else ""


def type_arguments(type_text: str) -> List[str]:
    open_at = type_text.find("[")
    if open_at < 0 or not type_text.strip().endswith("]"):
        return []
    return [part.strip() for part in split_top_level(type_text[open_at + 1 : type_text.rfind("]")])]


def element_type(type_text: str) -> str:
    args = type_arguments(type_text)
    return args[0] if base_type(type_text) in ITERABLE and len(args) == 1 else ""


def tensor_type(dimensions: List[int]) -> str:
    """A static tensor type for dimensions already read as numbers."""
    shape = ", ".join(str(dimension) for dimension in dimensions)
    if len(dimensions) == 1:
        shape += ","
    return f"Tensor[double, ({shape})]"


def concrete_type(type_text: str) -> str:
    """Replace a static tensor whose dimensions are only placeholders.

    Metadata signatures use symbolic dimensions such as `shape` or `(length,)`;
    they name a compiler variable, not a type the notebook can show. A shape
    that is not a literal tuple becomes the run-time `DynTensor` form.
    """
    text = type_text or ""

    def replace(match: re.Match[str]) -> str:
        parts = split_top_level(match.group(1))
        if len(parts) != 2:
            return "DynTensor[double]"
        shape = parts[1].strip()
        literal = re.fullmatch(r"\(\s*(?:\d[\d_]*(?:\s*,\s*\d[\d_]*)*\s*,?\s*)?\)", shape)
        return match.group(0) if literal else "DynTensor[double]"

    # The look-behind keeps `DynTensor[double]` from becoming `DynDynTensor[double]`.
    text = re.sub(r"(?<![A-Za-z_])Tensor\[([^\[\]]*)\]", replace, text)
    # Factorization metadata writes type variables such as `SVD[U, S, V]`;
    # completion only needs the class, and that spelling is not a value type.
    return re.sub(
        r"(?<![A-Za-z_])(LU|QR|SVD|Eigh)\[[^\[\]]*\]",
        r"\1",
        text,
    )


def result_of(signature: str, concrete: bool = True) -> str:
    """The result type in a metadata signature, without a trailing `with Effects`."""
    if ":" not in signature:
        return ""
    result = signature.rsplit(":", 1)[1].strip()
    result = re.sub(r"\s+with\s+Effects\b.*$", "", result).strip()
    return concrete_type(result) if concrete else result


def substitute(type_text: str, bindings: Dict[str, str]) -> str:
    """Replace bare generic names with the types read from a receiver."""
    result = type_text
    for name, value in bindings.items():
        result = re.sub(rf"\b{name}\b", value, result)
    return result


def unresolved(type_text: str) -> bool:
    """Whether a bare capital letter is still a generic variable."""
    return bool(re.search(r"(^|[^A-Za-z0-9_])[A-Z](?![A-Za-z0-9_])", type_text))


def error_union(type_text: str) -> bool:
    """`Series or Error` is not a Series: the `?` branch has not been taken yet."""
    return bool(re.search(r"\bor\s+Error\b", type_text or ""))


def receiver_bindings(actual: str, formal: str) -> Dict[str, str]:
    """Bind T from List[String] against List[T], and the same for other generics."""
    actual_base = base_type(actual)
    formal_base = base_type(formal)
    if formal_base == "Iterable" and actual_base in ITERABLE:
        actual_args = type_arguments(actual)
        return {"T": actual_args[0]} if len(actual_args) == 1 else {}
    if actual_base != formal_base:
        return {}
    actual_args = type_arguments(actual)
    formal_args = type_arguments(formal)
    if len(actual_args) != len(formal_args):
        return {}
    values: Dict[str, str] = {}
    for actual_arg, formal_arg in zip(actual_args, formal_args):
        name = formal_arg.strip()
        if re.fullmatch(r"[A-Z]", name):
            values[name] = actual_arg
    return values


def last_member_dot(text: str) -> int:
    """The last dot outside brackets and strings that is not a decimal point."""
    depth = 0
    found = -1
    i = 0
    while i < len(text):
        current = text[i]
        if current == '"':
            i = skip_string(text, i)
            continue
        if current in "([{":
            depth += 1
        elif current in ")]}":
            depth -= 1
        elif current == "." and depth == 0 and not (i and text[i - 1].isdigit() and i + 1 < len(text) and text[i + 1].isdigit()):
            found = i
        i += 1
    return found


def expression_end(text: str, start: int) -> int:
    """Where a top-level expression ends: a newline or semicolon outside brackets."""
    round_depth = square = curly = 0
    i = start
    while i < len(text):
        current = text[i]
        if current == '"':
            i = skip_string(text, i)
            continue
        if current == "(":
            round_depth += 1
        elif current == ")":
            round_depth -= 1
        elif current == "[":
            square += 1
        elif current == "]":
            square -= 1
        elif current == "{":
            curly += 1
        elif current == "}":
            curly -= 1
        elif round_depth == square == curly == 0 and current in "\n;":
            break
        i += 1
    return i


def split_binary(value: str) -> Optional[Tuple[str, str, str]]:
    """Split a top-level binary expression at its lowest-precedence operator."""
    found: List[Tuple[str, int]] = []
    depth = 0
    i = 0
    while i < len(value):
        current = value[i]
        if current == '"':
            i = skip_string(value, i)
            continue
        if current in "([{":
            depth += 1
        elif current in ")]}":
            depth -= 1
        elif depth == 0:
            token = next((op for op in OPERATORS if value.startswith(op, i)), None)
            if token in ("=>", "->"):
                i += 2
                continue
            if token:
                before = value[:i].rstrip()
                if before and (before[-1].isalnum() or before[-1] in ')]"'):
                    found.append((token, i))
                i += len(token)
                continue
        i += 1
    for level in (("|>",), ("||",), ("&&",), ("==", "!=", "<=", ">=", "<", ">"), ("+", "-"), ("*", "/", "%", "@"), ("**",)):
        candidates = [(op, at) for op, at in found if op in level]
        if not candidates:
            continue
        op, at = candidates[-1] if level != ("**",) else candidates[0]
        return op, value[:at].strip(), value[at + len(op) :].strip()
    return None


class Analyzer:
    """Reads just enough Kestrel to type notebook expressions and member chains."""

    def __init__(self, metadata: Optional[dict] = None):
        if metadata is None:
            path = Path(__file__).with_name("kesdas_metadata.json")
            try:
                metadata = json.loads(path.read_text())
            except (OSError, ValueError):
                # Completion is optional; a broken or missing table must not stop the kernel.
                metadata = {}
        self.metadata = metadata
        self.parents: Dict[str, str] = {}
        extra_path = Path(__file__).with_name("kesnet_metadata.json")
        try:
            extra = json.loads(extra_path.read_text())
            self.parents.update(extra.get("parents", {}))
        except (OSError, ValueError):
            pass
        self.methods: Dict[str, Dict[str, List[dict]]] = {}
        for receiver, entries in metadata.get("methods", {}).items():
            self.methods[receiver] = {}
            for entry in entries:
                self.methods[receiver].setdefault(entry["name"], []).append(entry)
        self.fields: Dict[str, Dict[str, str]] = metadata.get("fields", {})
        self.statics: Dict[str, Dict[str, List[dict]]] = {}
        for receiver, entries in metadata.get("statics", {}).items():
            self.statics[receiver] = {}
            for entry in entries:
                self.statics[receiver].setdefault(entry["name"], []).append(entry)
        self.constructors: Dict[str, List[dict]] = metadata.get("constructors", {})
        self.functions: Dict[str, List[dict]] = {}
        for entry in metadata.get("functions", []):
            self.functions.setdefault(entry["name"], []).append(entry)
        self.types = set(metadata.get("types", []))
        self.builtin_methods: Dict[str, Dict[str, List[dict]]] = {}
        self.builtin_fields: Dict[str, Dict[str, str]] = {}
        self.builtin_statics: Dict[str, Dict[str, List[dict]]] = {}
        self.builtin_constructors: Dict[str, List[dict]] = {}
        self.builtin_functions: Dict[str, List[dict]] = {}
        builtin_path = Path(__file__).with_name("builtins_metadata.json")
        try:
            builtin_metadata = json.loads(builtin_path.read_text())
        except (OSError, ValueError):
            builtin_metadata = {}
        for receiver, entries in builtin_metadata.get("methods", {}).items():
            group = self.builtin_methods.setdefault(base_type(receiver), {})
            for entry in entries:
                group.setdefault(entry["name"], []).append(entry)
        for receiver, entries in builtin_metadata.get("fields", {}).items():
            self.builtin_fields.setdefault(base_type(receiver), {}).update(entries)
        for receiver, entries in builtin_metadata.get("statics", {}).items():
            group = self.builtin_statics.setdefault(base_type(receiver), {})
            for entry in entries:
                group.setdefault(entry["name"], []).append(entry)
        for owner, entries in builtin_metadata.get("constructors", {}).items():
            self.builtin_constructors.setdefault(owner, []).extend(entries)
        for entry in builtin_metadata.get("functions", []):
            self.builtin_functions.setdefault(entry["name"], []).append(entry)
        # Every imported standard module and the names it exports.
        self.modules: Dict[str, List[str]] = {}
        for module in builtin_metadata.get("modules", []):
            path = module.get("path", "")
            if path:
                self.modules[path] = list(module.get("names", []))
        kesdas_module = self.metadata.get("module", "std.kesdas")
        self.modules[kesdas_module] = list(self.metadata.get("names", []))
        # Types can be recognised through a constructor, static members or a
        # metadata entry whose signature has no parameter list.
        self.builtin_types = set(self.builtin_statics) | set(self.builtin_constructors)
        self.builtin_types |= set(self.builtin_methods) | set(self.builtin_fields)
        for name, entries in self.builtin_functions.items():
            if any("(" not in entry.get("signature", "") for entry in entries):
                self.builtin_types.add(name)

    # ---- modules -----------------------------------------------------

    def module_names(self, path: str) -> List[str]:
        return self.modules.get(path, [])

    def module_of(self, receiver_text: str, env: Dict[str, str]) -> str:
        alias = env.get(receiver_text, "")
        return alias[len("@module:"):] if alias.startswith("@module:") else ""

    def is_type(self, name: str) -> bool:
        return name in self.types or name in self.builtin_types

    def name_detail(self, path: str, name: str) -> str:
        """The signature to show for one exported name."""
        for entry in self.constructors.get(name, []):
            return entry["signature"]
        for entry in self.builtin_constructors.get(name, []):
            if not path or entry.get("module") in ("", path):
                return entry["signature"]
        for entry in self.functions.get(name, []):
            return entry["signature"]
        for entry in self.builtin_functions.get(name, []):
            if not path or entry.get("module") in ("", path):
                return entry["signature"]
        return name

    def literal_shape(self, text: str) -> Tuple[str, Optional[List[int]]]:
        """A literal `(2, 3)` tuple or `[2, 3]` array of dimensions."""
        value = text.strip()
        if value.startswith("(") and value.endswith(")") and balanced(value[1:-1]):
            kind = "tuple"
            parts = split_top_level(value[1:-1])
        elif value.startswith("[") and value.endswith("]") and balanced(value[1:-1]):
            kind = "array"
            parts = split_top_level(value[1:-1])
        else:
            return "", None
        if not parts:
            return kind, [] if kind == "tuple" else None
        dimensions: List[int] = []
        for part in parts:
            stripped = part.strip().replace("_", "")
            if not re.fullmatch(r"\d+", stripped):
                return kind, None
            dimensions.append(int(stripped))
        return kind, dimensions

    def tensor_constructor(self, arguments: List[str]) -> str:
        """`Tensor([0, 0])` has a static length; an unknown length is dynamic."""
        if not arguments:
            return ""
        values = arguments[0].strip()
        if len(arguments) >= 2:
            kind, dimensions = self.literal_shape(arguments[1])
            if kind == "tuple" and dimensions is not None:
                return tensor_type(dimensions)
            return "DynTensor[double]"
        if values.startswith("[") and values.endswith("]") and balanced(values[1:-1]):
            count = len([part for part in split_top_level(values[1:-1]) if part.strip()])
            return tensor_type([count])
        return "DynTensor[double]"

    def tensor_factory(self, name: str, arguments: List[str]) -> str:
        if name in ("zeros", "ones", "filled", "generate") and arguments:
            kind, dimensions = self.literal_shape(arguments[0])
            if kind == "tuple" and dimensions is not None:
                return tensor_type(dimensions)
            return "DynTensor[double]"
        if name in ("zerosLike", "onesLike", "fromArray"):
            return "DynTensor[double]"
        return ""

    def conversion_result(self, name: str, arguments: List[str], env: Dict[str, str]) -> str:
        """A conversion used as a pipe target, such as `[0, 0] |> Tensor`."""
        if name == "Tensor":
            return self.tensor_constructor(arguments)
        if not arguments:
            return ""
        source = self.infer(arguments[0], env)
        if name == "double":
            return "double" if source in NUMERIC or base_type(source) in ("Tensor", "DynTensor") else ""
        if name == "String":
            return "String"
        if name == "Array":
            element = element_type(source)
            return f"Array[{element}]" if element else ""
        return ""

    def constructor_result(self, name: str, arguments: List[str], path: str = "") -> str:
        entries = [
            entry for entry in self.builtin_constructors.get(name, [])
            if not path or entry.get("module") in ("", path)
        ]
        if not entries:
            return ""
        if name == "Tensor":
            return self.tensor_constructor(arguments)
        return concrete_type(result_of(entries[0]["signature"]))

    def function_result(self, name: str, arguments: List[str], path: str = "") -> str:
        entries = [
            entry for entry in self.builtin_functions.get(name, [])
            if not path or entry.get("module") in ("", path)
        ]
        if not entries:
            return ""
        exact = [entry for entry in entries if len(entry.get("parameters", [])) == len(arguments)]
        return concrete_type(result_of((exact or entries)[0]["signature"]))

    # ---- type lookup -------------------------------------------------

    def type_chain(self, name: str) -> List[str]:
        chain: List[str] = []
        current = base_type(name)
        while current and current not in chain:
            chain.append(current)
            current = self.parents.get(current, "")
        return chain

    def method_entries(self, receiver: str, name: str) -> List[dict]:
        for owner in self.type_chain(receiver):
            entries = self.methods.get(owner, {}).get(name, [])
            if entries:
                return entries
        return []

    def field_type(self, receiver: str, name: str) -> str:
        if error_union(receiver):
            return ""
        for owner in self.type_chain(receiver):
            declared = self.fields.get(owner, {}).get(name, "")
            if declared:
                return concrete_type(declared)
        return concrete_type(self.builtin_field_type(receiver, name))

    def static_entries(self, owner: str, name: str) -> List[dict]:
        return self.statics.get(owner, {}).get(name, [])

    def method_result(self, receiver: str, name: str, arguments: List[str], env: Dict[str, str]) -> str:
        if error_union(receiver):
            return ""
        entries = self.method_entries(receiver, name)
        if not entries:
            return self.builtin_method_result(receiver, name, arguments)
        # Series.iloc gives a Scalar for a position and a Series for a list.
        if base_type(receiver) == "Series" and name == "iloc" and len(arguments) == 1:
            argument = self.infer(arguments[0], env)
            if base_type(argument) == "List":
                return "Series"
            return "Scalar" if argument else ""
        # Prefer the overload whose parameter count matches the call; default
        # parameters mean a shorter call can still use a longer signature.
        exact = [entry for entry in entries if len(entry.get("parameters", [])) == len(arguments)]
        return result_of((exact or entries)[0]["signature"])

    def member_signature(self, receiver: str, name: str) -> str:
        """A method signature for hover, from kesdas or the standard library."""
        if error_union(receiver):
            return ""
        entries = self.method_entries(receiver, name)
        if entries:
            return entries[0]["signature"] + ("\n\n" + entries[0]["documentation"] if entries[0].get("documentation") else "")
        base = base_type(receiver)
        groups = [self.builtin_methods.get(owner, {}) for owner in self.type_chain(receiver)]
        if base in ITERABLE:
            groups.append(self.builtin_methods.get("Iterable", {}))
        for group in groups:
            if name in group:
                entry = group[name][0]
                return entry["signature"] + ("\n\n" + entry["documentation"] if entry.get("documentation") else "")
        return ""

    def builtin_method_result(self, receiver: str, name: str, arguments: List[str]) -> str:
        """A standard-library method result, with its receiver generics filled in."""
        base = base_type(receiver)
        groups = [self.builtin_methods.get(owner, {}) for owner in self.type_chain(receiver)]
        # List and Array expose their own methods plus everything on Iterable.
        if base in ITERABLE:
            groups.append(self.builtin_methods.get("Iterable", {}))
        for group in groups:
            entries = group.get(name, [])
            if not entries:
                continue
            exact = [entry for entry in entries if len(entry.get("parameters", [])) == len(arguments)]
            for entry in exact or entries:
                bindings = receiver_bindings(receiver, entry.get("receiver", ""))
                result = substitute(result_of(entry["signature"], concrete=False), bindings)
                if result and not unresolved(result):
                    return concrete_type(result)
            return ""
        return ""

    def builtin_field_type(self, receiver: str, name: str) -> str:
        for owner in self.type_chain(receiver):
            declared = self.builtin_fields.get(owner, {}).get(name, "")
            if declared:
                return concrete_type(declared)
        return ""

    def builtin_static_result(
        self, owner: str, name: str, arguments: Optional[List[str]] = None, path: str = ""
    ) -> str:
        entries = [
            entry for entry in self.builtin_statics.get(owner, {}).get(name, [])
            if not path or entry.get("module") in ("", path)
        ]
        if not entries:
            return ""
        if owner == "Tensor":
            factory = self.tensor_factory(name, arguments or [])
            if factory:
                return factory
        return concrete_type(result_of(entries[0]["signature"]))

    def static_result(
        self, owner: str, name: str, arguments: Optional[List[str]] = None, path: str = ""
    ) -> str:
        if path in ("", self.metadata.get("module", "std.kesdas")):
            entries = self.static_entries(owner, name)
            if entries:
                return concrete_type(result_of(entries[0]["signature"]))
        return self.builtin_static_result(owner, name, arguments, path)

    # ---- expressions -------------------------------------------------

    def infer(self, expression: str, env: Dict[str, str], depth: int = 0) -> str:
        if depth > 24:
            return ""
        value = remove_outer_parens(expression.strip().rstrip(";").strip())
        if not value:
            return ""
        if re.match(r'^f?"(?:[^"\\]|\\.)*"$', value):
            return "String"
        if NUMBER.match(value):
            return "double" if ("." in value or "e" in value.lower()) and not value.lower().startswith("0x") else "Int"
        if value in ("true", "false"):
            return "Bool"
        if value == "None":
            return "Option[T]"
        if value.endswith("?"):
            inner = self.infer(value[:-1], env, depth + 1)
            if re.search(r"\s+or\s+Error$", inner):
                return re.sub(r"\s+or\s+Error$", "", inner)
            if base_type(inner) == "Option":
                args = type_arguments(inner)
                return args[0] if args else ""
            return ""
        if value.startswith("[") and value.endswith("]") and balanced(value[1:-1]):
            items = [self.infer(part, env, depth + 1) for part in split_top_level(value[1:-1]) if part.strip()]
            if items and all(items):
                unique = set(items)
                if len(unique) == 1:
                    return f"List[{items[0]}]"
                if unique <= {"Int", "double"}:
                    return "List[double]"
            return ""
        if value.startswith("(") and value.endswith(")") and balanced(value[1:-1]):
            parts = [self.infer(part, env, depth + 1) for part in split_top_level(value[1:-1]) if part.strip()]
            if parts and all(parts):
                return f"({', '.join(parts)})"
        if IDENTIFIER.match(value):
            if value in env:
                return env[value]
            if self.is_type(value):
                return value
            return ""
        split = split_binary(value)
        if split:
            op, left_text, right_text = split
            if op == "|>":
                name = right_text.strip()
                if name in ("Tensor", "Array", "double", "String"):
                    return self.conversion_result(name, [left_text], env)
                return result_of(self.functions[name][0]["signature"]) if name in self.functions else ""
            left = self.infer(left_text, env, depth + 1)
            right = self.infer(right_text, env, depth + 1)
            if not left or not right:
                return ""
            if error_union(left) or error_union(right):
                return ""
            if "Variable" in (base_type(left), base_type(right)):
                return "Variable" if op in ("+", "-", "*", "/", "@", "**") else ""
            if "Series" in (base_type(left), base_type(right)):
                if op == "+" and base_type(left) == "Series" and right == "String":
                    return "Series"
                if (base_type(left) == "Series" or left in NUMERIC) and (base_type(right) == "Series" or right in NUMERIC):
                    return "Series" if op in SERIES_OPS else ""
            if op in ("==", "!=", "<", "<=", ">", ">=", "&&", "||"):
                return "Bool"
            if left == right and left in NUMERIC:
                return left
            if left in NUMERIC and right in NUMERIC:
                return "double"
            if op == "+" and left == right == "String":
                return "String"
            return ""
        if value.startswith("-"):
            inner = self.infer(value[1:], env, depth + 1)
            return inner if inner in NUMERIC or base_type(inner) in ("Tensor", "DynTensor", "Complex", "Series", "Variable") else ""
        if value.endswith(")"):
            open_at = opener_of(value, len(value) - 1)
            if open_at < 0:
                return ""
            callee = value[:open_at].strip()
            raw = [part.strip() for part in split_top_level(value[open_at + 1 : -1]) if part.strip()]
            arguments = [NAMED_ARGUMENT.sub("", part).strip() for part in raw]
            dot = last_member_dot(callee)
            if dot >= 0:
                receiver_text = callee[:dot].strip()
                name = callee[dot + 1 :].strip()
                if not IDENTIFIER.match(name):
                    return ""
                if IDENTIFIER.match(receiver_text) and self.is_type(receiver_text):
                    return self.static_result(receiver_text, name, arguments)
                module = self.module_of(receiver_text, env)
                if module:
                    if name in self.constructors:
                        return name
                    if name in self.module_names(module) and self.is_type(name):
                        return name
                    if name in self.functions:
                        return result_of(self.functions[name][0]["signature"])
                    if name in self.builtin_constructors:
                        result = self.constructor_result(name, arguments, module)
                        if result:
                            return result
                    result = self.function_result(name, arguments, module)
                    if result:
                        return result
                owner = receiver_text.split(".", 1)
                if len(owner) == 2 and self.module_of(owner[0], env) and owner[1] in self.module_names(self.module_of(owner[0], env)):
                    return self.static_result(owner[1], name, arguments, self.module_of(owner[0], env))
                receiver = self.infer(receiver_text, env, depth + 1)
                return self.method_result(receiver, name, arguments, env) if receiver else ""
            if IDENTIFIER.match(callee):
                if callee in self.constructors:
                    return callee
                if callee in self.builtin_constructors:
                    result = self.constructor_result(callee, arguments)
                    return result if result and not unresolved(result) else callee
                if callee in self.functions:
                    return result_of(self.functions[callee][0]["signature"])
                if callee in self.builtin_functions:
                    return self.function_result(callee, arguments)
                return callee if self.is_type(callee) or callee in self.builtin_constructors else ""
            return ""
        dot = last_member_dot(value)
        if dot >= 0:
            receiver_text = value[:dot].strip()
            name = value[dot + 1 :].strip()
            if not IDENTIFIER.match(name):
                return ""
            if IDENTIFIER.match(receiver_text) and self.is_type(receiver_text):
                return self.static_result(receiver_text, name)
            owner = receiver_text.split(".", 1)
            if len(owner) == 2 and self.module_of(owner[0], env) and owner[1] in self.module_names(self.module_of(owner[0], env)):
                return self.static_result(owner[1], name, path=self.module_of(owner[0], env))
            receiver = self.infer(receiver_text, env, depth + 1)
            return self.field_type(receiver, name) if receiver else ""
        return ""

    # ---- notebook imports --------------------------------------------

    def imports(self, code: str) -> Dict[str, str]:
        """Namespace aliases from `import std.linalg` or `import std.linalg as la`."""
        found: Dict[str, str] = {}
        for line in strip_comments(code).splitlines():
            match = re.match(r"^\s*import\s+([A-Za-z_][A-Za-z0-9_.]*)\s*(?:as\s+([A-Za-z_][A-Za-z0-9_]*))?\s*$", line)
            if not match or "{" in line:
                continue
            path = match.group(1)
            alias = match.group(2) or path.rsplit(".", 1)[-1]
            if path in self.modules:
                found[alias] = f"@module:{path}"
        return found

    # ---- notebook bindings -------------------------------------------

    def bindings(self, code: str, env: Dict[str, str], offset: Optional[int] = None) -> List[dict]:
        """Top-level `let`/`var` bindings, with their inferred types."""
        clean = strip_comments(code)
        found: List[dict] = []
        current_env = dict(env)
        brace_depth = 0
        index = 0
        while index < len(clean):
            current = clean[index]
            if current == '"':
                index = skip_string(clean, index)
                continue
            if current == "{":
                brace_depth += 1
            elif current == "}":
                brace_depth -= 1
            if brace_depth == 0:
                # `match` at this exact position keeps a `let` inside a function or class body
                # out of the top-level environment; a search could jump over its opening brace.
                match = BINDING.match(clean, index)
                if match:
                    name = match.group(2)
                    rhs_start = match.end()
                    rhs_end = expression_end(clean, rhs_start)
                    annotation = (match.group(3) or "").strip()
                    rhs = code[rhs_start:rhs_end]
                    type_text = annotation or self.infer(rhs, current_env)
                    if type_text:
                        current_env[name] = type_text
                    found.append(
                        {
                            "name": name,
                            "type": type_text,
                            "name_start": match.start(2),
                            "rhs_start": rhs_start,
                            "rhs_end": rhs_end,
                            "annotation": annotation,
                        }
                    )
                    index = rhs_end
                    continue
            index += 1
        return found

    def env_after(self, code: str, env: Dict[str, str], offset: Optional[int] = None) -> Dict[str, str]:
        """The names visible after `code`, or before `offset` when one is given."""
        result = dict(env)
        result.update(self.imports(code))
        for binding in self.bindings(code, result):
            if offset is not None and binding["name_start"] >= offset:
                continue
            if binding["type"]:
                result[binding["name"]] = binding["type"]
        return result

    def member_receiver(self, code: str, offset: int) -> Tuple[str, str]:
        """The expression before a member dot and the prefix already typed."""
        before = code[:offset]
        match = re.search(r"\.\s*([A-Za-z0-9_]*)$", before)
        if not match:
            return "", ""
        end = before.rfind(".", 0, match.end())
        prefix = match.group(1)
        depth = 0
        i = end - 1
        while i >= 0:
            current = code[i]
            if current == '"':
                opened = code.rfind('"', 0, i)
                if opened < 0:
                    break
                i = opened - 1
                continue
            if current in ")]}":
                depth += 1
            elif current in "([{":
                if depth == 0:
                    break
                depth -= 1
            elif depth == 0 and current not in "?." and not (current.isalnum() or current == "_"):
                break
            i -= 1
        return code[i + 1 : end].strip(), prefix

    # ---- completion ---------------------------------------------------

    def member_candidates(self, receiver_text: str, env: Dict[str, str]) -> List[dict]:
        """Methods, fields and statics offered after `receiver_text.`."""
        module = self.module_of(receiver_text, env)
        if module:
            return [
                {"name": name, "detail": self.name_detail(module, name), "documentation": ""}
                for name in self.module_names(module)
            ]
        owner = receiver_text.split(".", 1)
        if len(owner) == 2:
            owner_module = self.module_of(owner[0], env)
            if owner_module and owner[1] in self.module_names(owner_module):
                entries = []
                groups = [
                    self.statics.get(owner[1], {}),
                    {
                        name: [
                            entry for entry in overloads
                            if entry.get("module") in ("", owner_module)
                        ]
                        for name, overloads in self.builtin_statics.get(owner[1], {}).items()
                    },
                ]
                for group in groups:
                    for name, overloads in group.items():
                        if overloads:
                            entries.append({
                                "name": name,
                                "detail": overloads[0]["signature"],
                                "documentation": overloads[0].get("documentation", ""),
                            })
                return entries
        if IDENTIFIER.match(receiver_text) and self.is_type(receiver_text):
            entries = []
            for name, group in (self.statics.get(receiver_text, {}) | self.builtin_statics.get(receiver_text, {})).items():
                entries.append({"name": name, "detail": group[0]["signature"], "documentation": group[0].get("documentation", "")})
            return entries
        type_text = self.infer(receiver_text, env)
        if error_union(type_text):
            return []
        base = base_type(type_text)
        if not base:
            return []
        entries = []
        for owner in self.type_chain(base):
            for name, group in self.methods.get(owner, {}).items():
                if name not in {entry["name"] for entry in entries}:
                    entries.append({"name": name, "detail": group[0]["signature"], "documentation": group[0].get("documentation", "")})
            for name, field_type in self.fields.get(owner, {}).items():
                if name not in {entry["name"] for entry in entries}:
                    entries.append({"name": name, "detail": f"{name} : {field_type}", "documentation": ""})
        groups = [self.builtin_methods.get(owner, {}) for owner in self.type_chain(base)]
        if base in ITERABLE:
            groups.append(self.builtin_methods.get("Iterable", {}))
        for group in groups:
            for name, overloads in group.items():
                if name not in {entry["name"] for entry in entries}:
                    entries.append({"name": name, "detail": overloads[0]["signature"], "documentation": overloads[0].get("documentation", "")})
        for owner in self.type_chain(base):
            for name, field_type in self.builtin_fields.get(owner, {}).items():
                if name not in {entry["name"] for entry in entries}:
                    entries.append({"name": name, "detail": f"{name} : {field_type}", "documentation": ""})
        return entries
