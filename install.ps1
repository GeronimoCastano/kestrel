# Installs Kestrel on Windows with everything it needs: the compiler, its own C toolchain,
# OpenBLAS for linear algebra and Z3 for `smt` proofs. Run it from PowerShell:
#
#   irm https://raw.githubusercontent.com/GeronimoCastano/kestrel/main/install.ps1 | iex
#
# It needs no administrator rights. It writes to %LOCALAPPDATA%\Programs\Kestrel, adds
# that folder's bin to your user PATH, and installs the VS Code extension when VS Code is
# on PATH. Run it again to update. Settings, all optional:
#   KESTREL_VERSION   a release such as 0.3.0, instead of the latest
#   KESTREL_HOME      the install folder
#   KESTREL_ARCHIVE   a downloaded release zip to install instead of fetching one

# The body runs in its own scope, so `irm | iex` leaves no settings behind in your shell.
& {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'
    # Windows PowerShell 5.1 may not offer TLS 1.2, which GitHub requires.
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    function Step($text) { Write-Host "  $text" }

    $repository = 'https://github.com/GeronimoCastano/kestrel'
    $asset = 'kestrel-x86_64-pc-windows-msvc.zip'
    $version = $env:KESTREL_VERSION
    $root = if ($env:KESTREL_HOME) { $env:KESTREL_HOME } else { Join-Path $env:LOCALAPPDATA 'Programs\Kestrel' }

    if (-not [Environment]::Is64BitOperatingSystem) {
        throw 'Kestrel needs 64-bit Windows.'
    }

    Write-Host 'Installing Kestrel'
    # Unpack beside the install folder, so the final move stays on one drive.
    $parent = Split-Path $root
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
    $work = Join-Path $parent ("kestrel-install-" + [Guid]::NewGuid())
    New-Item -ItemType Directory -Path $work | Out-Null
    try {
        $base = $null
        $vsix = $null
        $archive = Join-Path $work $asset
        if ($env:KESTREL_ARCHIVE) {
            Copy-Item $env:KESTREL_ARCHIVE $archive
        } else {
            $base = if ($version) { "$repository/releases/download/v$($version.TrimStart('v'))" } else { "$repository/releases/latest/download" }
            Step "Downloading $base/$asset"
            Invoke-WebRequest -UseBasicParsing -Uri "$base/$asset" -OutFile $archive
            $sumsFile = Join-Path $work 'SHA256SUMS'
            Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS" -OutFile $sumsFile
            $line = Get-Content $sumsFile | Where-Object { $_ -match [Regex]::Escape($asset) + '\s*$' } | Select-Object -First 1
            if (-not $line) { throw "SHA256SUMS does not list $asset." }
            $expected = ($line -split '\s+')[0].ToLower()
            $actual = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLower()
            if ($expected -ne $actual) { throw "The download does not match its checksum; try again." }
        }

        Step 'Unpacking'
        $unpacked = Join-Path $work 'unpacked'
        New-Item -ItemType Directory -Path $unpacked | Out-Null
        # Windows 10 and 11 ship bsdtar, which unpacks a large zip far faster than Expand-Archive.
        $tar = Join-Path $env:SystemRoot 'System32\tar.exe'
        if (Test-Path $tar) { & $tar -xf $archive -C $unpacked; if ($LASTEXITCODE) { throw 'Cannot unpack the archive.' } }
        else { Expand-Archive -Path $archive -DestinationPath $unpacked }
        $top = Get-ChildItem $unpacked -Directory | Select-Object -First 1
        if (-not (Test-Path (Join-Path $top.FullName 'bin\kestrel.exe'))) { throw 'The archive has no bin\kestrel.exe.' }

        # An editor keeps `kestrel serve` running; stop the old copy so its files can be replaced.
        Get-Process kestrel -ErrorAction SilentlyContinue |
            Where-Object { $_.Path -and $_.Path.StartsWith($root, [StringComparison]::OrdinalIgnoreCase) } |
            Stop-Process -Force -ErrorAction SilentlyContinue
        if (Test-Path $root) { Remove-Item -Recurse -Force $root }
        Move-Item $top.FullName $root
        Step "Installed to $root"
    } finally {
        Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
    }

    $bin = Join-Path $root 'bin'
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = @($userPath -split ';' | Where-Object { $_ })
    if ($entries -notcontains $bin) {
        [Environment]::SetEnvironmentVariable('Path', (@($bin) + $entries) -join ';', 'User')
        Step "Added $bin to your PATH"
    }
    if (($env:Path -split ';') -notcontains $bin) { $env:Path = "$bin;$env:Path" }

    # Programs built with Python interop load python313.dll when they start, so the
    # bundled CPython joins PATH too. It goes last, so a Python the user installed
    # still wins on `python`.
    $python = Join-Path $root 'python'
    if (Test-Path $python) {
        $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        $entries = @($userPath -split ';' | Where-Object { $_ })
        if ($entries -notcontains $python) {
            [Environment]::SetEnvironmentVariable('Path', (@($entries) + $python) -join ';', 'User')
            Step "Added $python to your PATH"
        }
        if (($env:Path -split ';') -notcontains $python) { $env:Path = "$env:Path;$python" }
    }

    $kestrel = Join-Path $bin 'kestrel.exe'
    $installed = (& $kestrel --version)

    # A first program proves the toolchain works and leaves nothing behind. Smart App
    # Control blocks unsigned programs it has never seen, and every program Kestrel
    # builds is one. The compiler still installs; the summary says what to change.
    $blocked = $false
    $check = Join-Path ([IO.Path]::GetTempPath()) ("kestrel-check-" + [Guid]::NewGuid())
    New-Item -ItemType Directory -Path $check | Out-Null
    try {
        Set-Content -Path (Join-Path $check 'hello.kes') -Value 'print("Hello, Kestrel!")' -Encoding ascii
        # Windows PowerShell turns a native command's stderr into errors, which 'Stop' would throw.
        $ErrorActionPreference = 'Continue'
        $greeting = (& $kestrel run (Join-Path $check 'hello.kes') 2>&1 | ForEach-Object { "$_" }) -join "`n"
        $ErrorActionPreference = 'Stop'
        if ($greeting -match 'Smart App Control') { $blocked = $true }
        elseif ($greeting.Trim() -ne 'Hello, Kestrel!') { throw "The installed compiler could not run a program: $greeting" }
    } finally {
        $ErrorActionPreference = 'Stop'
        Remove-Item -Recurse -Force $check -ErrorAction SilentlyContinue
    }

    # The release's own VSIX matches the compiler just installed; a local archive falls
    # back to the Marketplace listing.
    $code = Get-Command code -ErrorAction SilentlyContinue
    if ($code) {
        Step 'Installing the VS Code extension'
        $extension = 'kestrel-lang.kestrel-language'
        if ($base) {
            $vsix = Join-Path ([IO.Path]::GetTempPath()) ("kestrel-" + [Guid]::NewGuid() + '.vsix')
            try {
                Invoke-WebRequest -UseBasicParsing -Uri "$base/kestrel.vsix" -OutFile $vsix
                $extension = $vsix
            } catch {
                Step 'The release has no VSIX; using the Marketplace listing'
            }
        }
        & $code.Source --install-extension $extension --force | Out-Null
        if ($vsix) { Remove-Item $vsix -Force -ErrorAction SilentlyContinue }
    }

    Write-Host ''
    if ($blocked) {
        Write-Host "$installed is installed, but Windows blocked the first program it built." -ForegroundColor Yellow
        Write-Host 'Smart App Control, or another App Control policy on this computer, blocks unsigned'
        Write-Host 'programs it has not seen before, and every program Kestrel builds is new.'
        Write-Host '`kestrel check` works. To run programs, install Kestrel in WSL, which Smart App Control'
        Write-Host 'leaves alone, or turn Smart App Control off in Windows Security > App & browser control >'
        Write-Host 'Smart App Control settings. Steps for both:'
        Write-Host '  https://github.com/GeronimoCastano/kestrel/blob/main/docs/install.md#smart-app-control'
    } else {
        Write-Host "$installed is ready."
        Write-Host 'Try it in a new terminal:  kestrel run hello.kes'
    }
    if (-not $code) {
        Write-Host 'For editor support, install "Kestrel for VS Code" from the Extensions view.'
    }
}
