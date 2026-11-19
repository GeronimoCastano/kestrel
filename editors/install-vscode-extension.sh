#!/bin/sh
# Build and install the local Kestrel VS Code extension.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/kestrel-vsix.XXXXXX")
trap 'rm -rf "$work"' EXIT INT TERM
mkdir -p "$work/extension"
cp -R "$root/editors/vscode-kestrel/." "$work/extension/"
cat > "$work/[Content_Types].xml" <<'XML'
<?xml version="1.0" encoding="utf-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="json" ContentType="application/json"/>
  <Default Extension="md" ContentType="text/markdown"/>
  <Default Extension="js" ContentType="application/javascript"/>
  <Default Extension="vsixmanifest" ContentType="text/xml"/>
</Types>
XML
cat > "$work/extension.vsixmanifest" <<'XML'
<?xml version="1.0" encoding="utf-8"?>
<PackageManifest Version="2.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011">
  <Metadata>
    <Identity Language="en-US" Id="kestrel" Version="0.2.0" Publisher="kestrel-lang"/>
    <DisplayName>Kestrel</DisplayName>
    <Description xml:space="preserve">Syntax highlighting for the Kestrel language</Description>
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
python3 - "$work" "$root/editors/kestrel.vsix" <<'PY'
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
code --install-extension "$root/editors/kestrel.vsix" --force
echo "Installed Kestrel. Reload VS Code to activate the extension."
