# Builds and (optionally) signs the Specular MSI. ./build.sh release windows --package does the
# same from Git Bash; this is for PowerShell alone.
#
#   pwsh packaging/windows/build-msi.ps1 [-Version 0.1.0] [-BinDir target\release]
#
# Builds the binaries first when BinDir lacks them, which needs what README.md lists for Windows:
# with ninja and clang-cl on PATH, libjxl is built with Ninja (CMAKE_GENERATOR).
#
# Signing: set SPECULAR_SIGN_CERT_SHA1 (thumbprint of a code-signing certificate in the user's
# store, e.g. an Azure Trusted Signing / EV certificate) to sign specular.exe,
# specular-decode-host.exe and the MSI with signtool. Without it the MSI is unsigned (SmartScreen
# will warn).
param(
    [string]$Version = "",
    [string]$BinDir = "target\release"
)
$ErrorActionPreference = "Stop"
$Root = Resolve-Path "$PSScriptRoot\..\.."
Set-Location $Root
if (-not $Version) {
    $Version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
}
New-Item -ItemType Directory -Force target\dist | Out-Null

# $ErrorActionPreference does not cover native programs: stop on their exit codes too.
function Check($what) {
    if ($LASTEXITCODE) { throw "$what failed (exit $LASTEXITCODE)" }
}

# The MSI also installs the sandboxed decode host (specular.wxs, DecodeHost): the viewer starts it
# from its own folder.
if (-not (Test-Path "$BinDir\specular.exe") -or -not (Test-Path "$BinDir\specular-decode-host.exe")) {
    if (-not $env:CMAKE_GENERATOR -and (Get-Command ninja -ErrorAction SilentlyContinue) -and
        (Get-Command clang-cl -ErrorAction SilentlyContinue)) {
        $env:CMAKE_GENERATOR = "Ninja"
    }
    # specular/asm: rav1d's and rav1e's assembly for AVIF decode and output (needs nasm on PATH).
    cargo build --locked --release --features specular/asm -p specular --bin specular -p specular-decode-host --bin specular-decode-host
    Check "cargo build"
}

function Sign($path) {
    if ($env:SPECULAR_SIGN_CERT_SHA1) {
        signtool sign /sha1 $env:SPECULAR_SIGN_CERT_SHA1 /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $path
        Check "signtool"
    }
}
Sign "$BinDir\specular.exe"
Sign "$BinDir\specular-decode-host.exe"

if (-not (Get-Command wix -ErrorAction SilentlyContinue)) {
    dotnet tool install -g wix --version "4.*"
    Check "dotnet tool install wix"
}
# The UI extension pinned to wix's own version ("4.0.6+hash" -> 4.0.6), when adding and when
# building: unpinned, the newest is fetched and used, which an older wix cannot load (WIX0144).
$Ui = "WixToolset.UI.wixext/" + ((wix --version) -split '\+')[0].Trim()
wix extension add -g $Ui
Check "wix extension add"
$Msi = "target\dist\Specular-$Version-x64.msi"
wix build packaging\windows\specular.wxs -arch x64 -ext $Ui `
    -d Version=$Version -d BinDir=$BinDir -d PackagingDir=packaging -o $Msi
Check "wix build"
Sign $Msi
# "<hash>  <file name>", as sha256sum writes it, so `sha256sum -c` checks it like the others.
"{0}  {1}" -f (Get-FileHash $Msi -Algorithm SHA256).Hash.ToLower(), (Split-Path $Msi -Leaf) |
    Out-File "$Msi.sha256" -Encoding ascii
Write-Host "built $Msi"
