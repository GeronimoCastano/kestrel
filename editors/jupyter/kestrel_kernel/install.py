"""Installs the Kestrel kernel spec: python3 -m kestrel_kernel.install [--user|--prefix DIR]."""

from __future__ import annotations

import argparse
import json
import shutil
import sys
import tempfile
from pathlib import Path

from jupyter_client.kernelspec import KernelSpecManager

HERE = Path(__file__).resolve().parent


def install(user: bool = True, prefix: str | None = None, executable: str | None = None) -> str:
    spec = {
        "argv": [sys.executable, "-m", "kestrel_kernel", "-f", "{connection_file}"],
        "display_name": "Kestrel",
        "language": "kestrel",
        "interrupt_mode": "signal",
    }
    if executable:
        spec["env"] = {"KESTREL": executable}
    with tempfile.TemporaryDirectory() as folder:
        target = Path(folder)
        (target / "kernel.json").write_text(json.dumps(spec, indent=2))
        for name in ("logo-32x32.png", "logo-64x64.png"):
            shutil.copy(HERE / name, target / name)
        return KernelSpecManager().install_kernel_spec(
            str(target), "kestrel", user=user if prefix is None else False, prefix=prefix
        )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--user", action="store_true", help="install for the current user (default)")
    parser.add_argument("--prefix", help="install under this prefix instead")
    parser.add_argument("--kestrel", help="path of the kestrel executable the kernel should run")
    arguments = parser.parse_args()
    print("Installed the Kestrel kernel spec at", install(True, arguments.prefix, arguments.kestrel))


if __name__ == "__main__":
    main()
