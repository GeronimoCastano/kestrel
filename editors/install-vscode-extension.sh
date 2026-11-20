#!/bin/sh
# Build and install the Kestrel VS Code extension from this checkout.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
"$root/editors/package-vscode-extension.sh"
code --install-extension "$root/editors/kestrel.vsix" --force
echo "Installed Kestrel. Reload VS Code to activate the extension."
