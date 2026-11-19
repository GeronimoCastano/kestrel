"""The Kestrel kernel for Jupyter."""

from __future__ import annotations

import asyncio
import re

from ipykernel.kernelbase import Kernel

from . import __version__
from .analysis import Analyzer
from .host import Host

KEYWORDS = [
    "let", "mut", "const", "function", "class", "enum", "trait", "alias", "import", "public",
    "private", "package", "static", "final", "var", "new", "this", "match", "if", "else",
    "then", "for", "in", "while", "break", "continue", "return", "using", "scope", "spawn",
    "move", "extends", "has", "requires", "ensures", "unsafe", "true", "false", "otherwise",
]

WORD = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")

# Jupyter clients render the traceback of an error output as ANSI text, which is how a
# Python kernel's traceback gets its colours. The compiler's diagnostics are plain text,
# so the kernel marks the parts a reader needs: the error line, its carets and its hint.
RESET = "\x1b[0m"
ERROR_COLOUR = "\x1b[1;31m"
MARK_COLOUR = "\x1b[31m"
HINT_COLOUR = "\x1b[33m"
DIAGNOSTIC = re.compile(r"^.*?:\d+:\d+: (?:error|warning)\[")
STOP = re.compile(r"^.*?:\d+:\d+: runtime error:")
CARET = re.compile(r"^\s*(?:\d+\s*)?\|\s*\^[\s^]*$")
HINT = re.compile(r"^\s*(?:hint|note|help):")


def coloured(value: str) -> list:
    """The error's lines, with the diagnostic parts a Python traceback colours."""
    lines = []
    for line in value.split("\n"):
        if "\x1b[" in line:
            lines.append(line)
        elif DIAGNOSTIC.match(line) or STOP.match(line):
            lines.append(f"{ERROR_COLOUR}{line}{RESET}")
        elif CARET.match(line):
            lines.append(f"{MARK_COLOUR}{line}{RESET}")
        elif HINT.match(line):
            lines.append(f"{HINT_COLOUR}{line}{RESET}")
        else:
            lines.append(line)
    return lines


class KestrelKernel(Kernel):
    implementation = "Kestrel"
    implementation_version = __version__
    language = "kestrel"
    language_version = "0.1"
    language_info = {
        "name": "kestrel",
        "mimetype": "text/x-kestrel",
        "file_extension": ".kes",
        "codemirror_mode": "javascript",
        "pygments_lexer": "text",
    }
    banner = "Kestrel: a statically typed language. Import std.kesdas for pandas-style tables."

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.host = Host()
        self.seen_words: set = set()
        self.analyzer = Analyzer()
        self.env: dict = {}

    async def _ensure_host(self) -> None:
        if not self.host.alive:
            await self.host.stop()
            self.host = Host()
            # A new native session no longer has the old values or names.
            self.env = {}
            self.seen_words = set()
            await self.host.start()

    def _stream(self, name: str, text: str) -> None:
        self.send_response(self.iopub_socket, "stream", {"name": name, "text": text})

    def _error(self, name: str, value: str) -> dict:
        # The first line is also the evalue, which clients show as plain text.
        first = value.split("\n")[0] if value else ""
        trace = coloured(value)
        self.send_response(
            self.iopub_socket,
            "error",
            {"ename": name, "evalue": first, "traceback": trace},
        )
        return {
            "status": "error",
            "ename": name,
            "evalue": first,
            "traceback": trace,
            "execution_count": self.execution_count,
        }

    async def do_execute(
        self, code, silent, store_history=True, user_expressions=None, allow_stdin=False, *, cell_meta=None, cell_id=None
    ):
        if not code.strip():
            return self._ok()
        try:
            await self._ensure_host()
        except Exception as problem:  # the executable is missing or cannot start
            return self._error("KestrelStartError", str(problem))
        self.seen_words.update(WORD.findall(code))
        failure = None
        status = "dead"
        pending_out: list = []
        try:
            async for event in self.host.run(code):
                if event.kind == "stdout":
                    pending_out.append(event.text)
                    if sum(len(t) for t in pending_out) > 4096:
                        self._flush(pending_out, silent)
                elif event.kind == "stderr":
                    self._flush(pending_out, silent)
                    if not silent:
                        self._stream("stderr", event.text)
                elif event.kind == "result":
                    self._flush(pending_out, silent)
                    if not silent:
                        self.send_response(
                            self.iopub_socket,
                            "execute_result",
                            {"execution_count": self.execution_count, "data": event.data, "metadata": {}},
                        )
                elif event.kind == "error":
                    self._flush(pending_out, silent)
                    failure = event.text
                elif event.kind == "done":
                    status = event.status
        except (asyncio.CancelledError, KeyboardInterrupt):
            self.host.kill()
            self._flush(pending_out, silent)
            return self._error("KeyboardInterrupt", "The cell was interrupted. The session was reset.")
        self._flush(pending_out, silent)
        if status == "dead":
            await self.host.stop()
            return self._error(
                "SessionEnded",
                (failure + "\n" if failure else "")
                + "The Kestrel session stopped, so its values are gone. The next cell starts a new session.",
            )
        if failure is not None:
            return self._error("KestrelError", failure)
        # Keep the names and types the cell bound, for completion in later cells.
        self.env = self.analyzer.env_after(code, self.env)
        return self._ok()

    def _flush(self, pending: list, silent: bool) -> None:
        if pending and not silent:
            self._stream("stdout", "".join(pending))
        pending.clear()

    def _ok(self) -> dict:
        return {
            "status": "ok",
            "execution_count": self.execution_count,
            "payload": [],
            "user_expressions": {},
        }

    async def do_is_complete(self, code):
        try:
            await self._ensure_host()
            status = await self.host.check(code)
        except Exception:
            return {"status": "unknown"}
        if status == "incomplete":
            return {"status": "incomplete", "indent": "    "}
        return {"status": "complete" if status == "complete" else "unknown"}

    def do_complete(self, code, cursor_pos):
        before = code[:cursor_pos]
        prefix_match = re.search(r"([A-Za-z_][A-Za-z0-9_]*)$", before)
        prefix = prefix_match.group(1) if prefix_match else ""
        try:
            env = self.analyzer.env_after(before, self.env)
            receiver, member_prefix = self.analyzer.member_receiver(code, cursor_pos)
            if member_prefix:
                prefix = member_prefix
            pool = {entry["name"] for entry in self.analyzer.member_candidates(receiver, env)} if receiver else set()
        except Exception:
            # Completion must never take the kernel down; fall back to words.
            env, receiver, pool = {}, "", set()
        if not receiver:
            pool = (
                set(KEYWORDS)
                | self.seen_words
                | set(WORD.findall(before))
                | set(env)
                | set(self.analyzer.types)
                | set(self.analyzer.functions)
                | {"print", "range", "Map", "Set", "String", "double", "Int", "Bool", "Array", "Tensor", "DynTensor"}
            )
        start = cursor_pos - len(prefix)
        matches = sorted(word for word in pool if word.startswith(prefix) and word != prefix)
        return {
            "status": "ok",
            "matches": matches,
            "cursor_start": start,
            "cursor_end": cursor_pos,
            "metadata": {},
        }

    def do_inspect(self, code, cursor_pos, detail_level=0, omit_sections=()):
        before = code[:cursor_pos]
        left = re.search(r"([A-Za-z_][A-Za-z0-9_]*)$", before)
        right = re.match(r"[A-Za-z0-9_]*", code[cursor_pos:])
        if not left:
            return {"status": "ok", "found": False, "data": {}, "metadata": {}}
        try:
            name = left.group(1) + (right.group(0) if right else "")
            name_end = cursor_pos + (len(right.group(0)) if right else 0)
            receiver, _ = self.analyzer.member_receiver(code, name_end)
            env = self.analyzer.env_after(before, self.env)
            text = ""
            if receiver:
                receiver_type = self.analyzer.infer(receiver, env)
                text = self.analyzer.member_signature(receiver_type, name)
                if not text and self.analyzer.field_type(receiver_type, name):
                    text = f"{name} : {self.analyzer.field_type(receiver_type, name)}"
            elif name in env:
                text = f"{name} : {env[name]}"
        except Exception:
            return {"status": "ok", "found": False, "data": {}, "metadata": {}}
        if not text:
            return {"status": "ok", "found": False, "data": {}, "metadata": {}}
        return {"status": "ok", "found": True, "data": {"text/plain": text}, "metadata": {}}

    async def do_shutdown(self, restart):
        self.host.close()
        return {"status": "ok", "restart": restart}
