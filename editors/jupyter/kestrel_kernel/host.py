"""The client side of `kestrel notebook`.

The host reads requests on standard input and writes marked blocks on standard output.
Every control line starts with the ASCII record separator (RS, 0x1e):

    RS[result] ... RS[text/plain] text ... RS[text/html] html ... RS[end]   a cell's value
    RS[error] message RS[end]                                              a failure
    RS[done] ID STATUS                                                     the cell is finished

Anything outside a block is ordinary program output. The host repeats the `[done]` line on
standard error, so a reader that has seen it on both streams has everything the cell wrote.
"""

from __future__ import annotations

import asyncio
import os
import shutil
from dataclasses import dataclass, field
from pathlib import Path
from typing import AsyncIterator, Optional

RS = "\x1e"


@dataclass
class Event:
    """One thing a cell produced."""

    kind: str  # "stdout", "stderr", "result", "error", "done"
    text: str = ""
    data: dict = field(default_factory=dict)
    status: str = ""


def find_kestrel() -> str:
    """The kestrel executable: $KESTREL, then PATH, then a build next to this checkout."""
    given = os.environ.get("KESTREL")
    if given:
        return given
    found = shutil.which("kestrel")
    if found:
        return found
    here = Path(__file__).resolve()
    for parent in here.parents:
        for build in ("release", "debug"):
            candidate = parent / "target" / build / "kestrel"
            if candidate.is_file():
                return str(candidate)
    raise FileNotFoundError(
        "Cannot find the kestrel executable. Put it on PATH or set KESTREL to its path."
    )


class Host:
    """A running `kestrel notebook` process."""

    def __init__(self, executable: Optional[str] = None, root: Optional[str] = None):
        self.executable = executable or find_kestrel()
        self.root = root or os.getcwd()
        self.process: Optional[asyncio.subprocess.Process] = None
        self.counter = 0

    @property
    def alive(self) -> bool:
        return self.process is not None and self.process.returncode is None

    async def start(self) -> None:
        extra = os.environ.get("KESTREL_LINK", "").split()
        args = [self.executable, "notebook", "--root", self.root]
        for library in extra:
            args += ["--link", library]
        self.process = await asyncio.create_subprocess_exec(
            *args,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            limit=1 << 26,
        )

    async def stop(self) -> None:
        process = self.process
        if process is None:
            return
        self.process = None
        if process.returncode is None:
            try:
                process.stdin.write(b"EXIT\n")
                await process.stdin.drain()
                await asyncio.wait_for(process.wait(), timeout=5)
            except Exception:
                process.kill()
                await process.wait()

    def close(self) -> None:
        """Ends the process without awaiting it, so it can run on any event loop."""
        process = self.process
        self.process = None
        if process is not None and process.returncode is None:
            try:
                process.stdin.write(b"EXIT\n")
            except Exception:
                pass
            process.kill()

    def kill(self) -> None:
        if self.process is not None and self.process.returncode is None:
            self.process.kill()

    async def _request(self, kind: str, code: str) -> str:
        assert self.process is not None and self.process.stdin is not None
        self.counter += 1
        ident = str(self.counter)
        payload = code.encode("utf-8")
        self.process.stdin.write(f"{kind} {ident} {len(payload)}\n".encode() + payload)
        await self.process.stdin.drain()
        return ident

    async def check(self, code: str) -> str:
        """Whether the code is a complete entry: "complete", "incomplete" or "dead"."""
        ident = await self._request("CHECK", code)
        status = "dead"
        async for event in self._events(ident):
            if event.kind == "done":
                status = event.status
        return status

    async def run(self, code: str) -> AsyncIterator[Event]:
        """Runs a cell and yields what it produces, ending with a "done" event."""
        ident = await self._request("CELL", code)
        async for event in self._events(ident):
            yield event

    async def _events(self, ident: str) -> AsyncIterator[Event]:
        process = self.process
        assert process is not None and process.stdout and process.stderr
        queue: asyncio.Queue = asyncio.Queue()

        async def pump(stream, name: str) -> None:
            while True:
                line = await stream.readline()
                if not line:
                    await queue.put((name, None))
                    return
                text = line.decode("utf-8", "replace")
                await queue.put((name, text))
                if text.startswith(f"{RS}[done] {ident} "):
                    await queue.put((name, None))
                    return

        tasks = [
            asyncio.ensure_future(pump(process.stdout, "out")),
            asyncio.ensure_future(pump(process.stderr, "err")),
        ]
        finished = 0
        status = "dead"
        block: Optional[str] = None  # "result" or "error"
        part = "text/plain"
        parts: dict = {}
        error_lines: list = []
        try:
            while finished < 2:
                name, text = await queue.get()
                if text is None:
                    finished += 1
                    continue
                if name == "err":
                    if not text.startswith(RS):
                        yield Event("stderr", text)
                    continue
                if not text.startswith(RS):
                    if block == "result":
                        parts.setdefault(part, []).append(text)
                    elif block == "error":
                        error_lines.append(text)
                    else:
                        yield Event("stdout", text)
                    continue
                marker = text.rstrip("\n")
                if marker == f"{RS}[result]":
                    block, parts, part = "result", {}, "text/plain"
                elif marker == f"{RS}[error]":
                    block, error_lines = "error", []
                elif marker.startswith(f"{RS}[text/"):
                    part = marker[len(RS) + 1 : -1]
                elif marker == f"{RS}[end]":
                    if block == "result":
                        data = {mime: "".join(lines).rstrip("\n") for mime, lines in parts.items()}
                        yield Event("result", data=data)
                    elif block == "error":
                        yield Event("error", "".join(error_lines).rstrip("\n"))
                    block = None
                elif marker.startswith(f"{RS}[done] "):
                    status = marker.split()[-1]
        finally:
            for task in tasks:
                task.cancel()
        yield Event("done", status=status)
