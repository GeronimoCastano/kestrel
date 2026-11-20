#!/bin/sh
# Build the VS Code extension package (.vsix) without installing it.
#
# Usage: editors/package-vscode-extension.sh [OUTPUT_PATH]
# The default output is editors/kestrel.vsix.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
output=${1:-"$root/editors/kestrel.vsix"}
work=$(mktemp -d "${TMPDIR:-/tmp}/kestrel-vsix.XXXXXX")
trap 'rm -rf "$work"' EXIT INT TERM
mkdir -p "$work/extension"
# Package only what the editor needs at run time. The patterns mirror
# editors/vscode-kestrel/.vscodeignore and drop local build output such as
# stray .vsix files and .DS_Store.
tar -C "$root/editors/vscode-kestrel" -cf - \
    --exclude='.DS_Store' --exclude='*.vsix' --exclude='node_modules' \
    --exclude='src' --exclude='build' --exclude='test' --exclude='test-support' \
    --exclude='package-lock.json' --exclude='BENCHMARKS.md' --exclude='.vscodeignore' \
    . | tar -C "$work/extension" -xf -

# The manifest carries the version from package.json, so the package cannot
# claim a version the extension does not have.
version=$(python3 - "$root/editors/vscode-kestrel/package.json" <<'PY'
import json, sys
print(json.load(open(sys.argv[1]))["version"])
PY
)

cat > "$work/[Content_Types].xml" <<'XML'
<?xml version="1.0" encoding="utf-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="json" ContentType="application/json"/>
  <Default Extension="md" ContentType="text/markdown"/>
  <Default Extension="js" ContentType="application/javascript"/>
  <Default Extension="vsixmanifest" ContentType="text/xml"/>
</Types>
XML

cat > "$work/extension.vsixmanifest" <<XML
<?xml version="1.0" encoding="utf-8"?>
<PackageManifest Version="2.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011">
  <Metadata>
    <Identity Language="en-US" Id="kestrel" Version="$version" Publisher="kestrel-lang"/>
    <DisplayName>Kestrel</DisplayName>
    <Description xml:space="preserve">Kestrel language support: highlighting, completions, hover, go to definition, inlay hints, proof state and quick fixes</Description>
    <Categories>Programming Languages</Categories>
    <Properties>
      <Property Id="Microsoft.VisualStudio.Code.Engine" Value="^1.70.0"/>
    </Properties>
  </Metadata>
  <Installation>
    <InstallationTarget Id="Microsoft.VisualStudio.Code"/>
  </Installation>
  <Dependencies/>
  <Assets>
    <Asset Type="Microsoft.VisualStudio.Code.Manifest" Path="extension/package.json" Addressable="true"/>
  </Assets>
</PackageManifest>
XML

python3 - "$work" "$output" <<'PY'
import os, sys, zipfile
work, output = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED) as archive:
    for base in ("[Content_Types].xml", "extension.vsixmanifest"):
        archive.write(os.path.join(work, base), base)
    extension = os.path.join(work, "extension")
    for directory, _, files in os.walk(extension):
        for name in files:
            path = os.path.join(directory, name)
            archive.write(path, os.path.relpath(path, work))
PY
echo "Packaged Kestrel $version: $output"
