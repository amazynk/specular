# Installs the MSI, checks what it put on the system, runs the installed viewer, uninstalls it
# and checks what is left. The release workflow runs this on the package it just built, so a
# package that does not install or a viewer that does not run never reaches a release.
# sandbox\run.ps1 is the thorough check (the portable zip, the jump list, file names in other
# scripts) and needs Windows Sandbox; this one needs only administrator rights and changes the
# machine it runs on, so use a CI runner or a virtual machine.
#
#   pwsh packaging/windows/smoke-test.ps1 MSI [OUT_DIR]
#
# OUT_DIR (default: a temporary folder) gets the installer's logs and the viewer's output.
# Each line printed is one check: "ok" or "FAIL", or "info" for what is reported without
# judging (a runner has no graphics card, so what the window does there says little).
# This file stays ASCII (Windows PowerShell reads it as ANSI).
param(
    [Parameter(Mandatory)][string]$Msi,
    [string]$Out = (Join-Path ([IO.Path]::GetTempPath()) "specular-smoke-$PID")
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Msi = (Resolve-Path $Msi).Path
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path

$progId = 'Specular.Image'
$dir = Join-Path $env:ProgramFiles 'Specular'
$exe = Join-Path $dir 'specular.exe'
$classes = 'HKLM:\Software\Classes'
$shortcut = Join-Path $env:ProgramData 'Microsoft\Windows\Start Menu\Programs\Specular.lnk'

$failed = 0
function Check([string]$what, [bool]$ok) {
    if ($ok) {
        Write-Host "ok    $what"
    } else {
        Write-Host "FAIL  $what"
        $script:failed++
    }
}
function Info([string]$what) { Write-Host "info  $what" }

# A registry value, or $null when the key or the value is missing ('' is the default value).
function RegValue([string]$key, [string]$value) {
    if (-not (Test-Path -LiteralPath $key)) { return $null }
    (Get-Item -LiteralPath $key).GetValue($value, $null)
}

function Msiexec([string[]]$arguments, [string]$log) {
    $p = Start-Process msiexec.exe -Wait -PassThru `
        -ArgumentList ($arguments + @('/qn', '/norestart', '/l*v', "`"$log`""))
    if ($p.ExitCode -ne 0) {
        Get-Content $log -Tail 80 | Write-Host
        throw "msiexec $($arguments -join ' ') ended with $($p.ExitCode)"
    }
}

# Runs the installed viewer with its output in files (a GUI program has no console to print
# to) and waits up to $seconds for it. Returns the exit code, or $null after a timeout.
function RunApp([string[]]$arguments, [string]$tag, [int]$seconds) {
    $env:SPECULAR_LOG_FILE = Join-Path $Out "$tag.log"
    $quoted = $arguments | ForEach-Object { if ($_ -match '\s') { "`"$_`"" } else { $_ } }
    $p = Start-Process $exe -ArgumentList $quoted -PassThru -NoNewWindow `
        -RedirectStandardOutput (Join-Path $Out "$tag.out.txt") `
        -RedirectStandardError (Join-Path $Out "$tag.err.txt")
    # Without this the exit code of a process started this way reads as empty.
    $null = $p.Handle
    if (-not $p.WaitForExit($seconds * 1000)) {
        Stop-Process -Id $p.Id -Force
        return $null
    }
    $p.ExitCode
}

# The Windows subsystem of an executable: 2 is a GUI program, 3 a console program.
function Subsystem([string]$file) {
    $bytes = [IO.File]::ReadAllBytes($file)
    $pe = [BitConverter]::ToInt32($bytes, 0x3C)
    # The optional header starts 24 bytes after the PE signature; Subsystem is at 68 in it.
    [BitConverter]::ToUInt16($bytes, $pe + 24 + 68)
}

Write-Host "== Install $Msi"
$jpgBefore = RegValue "$classes\.jpg" ''
Msiexec @('/i', "`"$Msi`"") (Join-Path $Out 'install.log')
Check "the viewer is in $dir" (Test-Path -LiteralPath $exe)
if (-not (Test-Path -LiteralPath $exe)) { throw 'nothing to test' }
Check 'the decode host is beside it' (Test-Path -LiteralPath (Join-Path $dir 'specular-decode-host.exe'))

Write-Host '== The executable'
Check 'it is a GUI program: starting it opens no console window' ((Subsystem $exe) -eq 2)

Write-Host '== What the installer registered'
$command = RegValue "$classes\$progId\shell\open\command" ''
Check 'its file types open with the viewer' ($command -and $command.Contains($exe))
Check 'the viewer is offered under Open with for .jpg' `
    ((Test-Path "$classes\.jpg\OpenWithProgids") -and
     ((Get-Item "$classes\.jpg\OpenWithProgids").GetValueNames() -contains $progId))
Check 'the viewer is listed in Settings > Default apps' `
    ((RegValue 'HKLM:\Software\RegisteredApplications' 'Specular') -eq 'SOFTWARE\Specular\Capabilities')
Check 'the handler of .jpg for all users is left as it was' ((RegValue "$classes\.jpg" '') -eq $jpgBefore)
Check 'the Start menu has a shortcut' (Test-Path -LiteralPath $shortcut)

Write-Host '== The viewer runs'
$code = RunApp @('--version') 'version' 60
$version = Get-Content (Join-Path $Out 'version.out.txt') -Raw -ErrorAction SilentlyContinue
Info "--version: $version"
Check '--version prints the version and ends with 0' (($code -eq 0) -and ($version -match '\d+\.\d+\.\d+'))

# Thumbnails are decoded and scaled on the processor: no graphics card needed.
$samples = @{
    jpg = Join-Path $root 'crates\formats\specular-fmt-thermal\tests\data\flir_noraw.jpg'
    jxl = Join-Path $root 'crates\formats\specular-jxl\tests\data\alpha.jxl'
}
foreach ($kind in $samples.Keys) {
    $png = Join-Path $Out "thumb-$kind.png"
    $code = RunApp @('--thumbnail', $samples[$kind], $png, '256') "thumb-$kind" 120
    Check "a thumbnail of a .$kind file is written" (($code -eq 0) -and (Test-Path -LiteralPath $png))
}

# What the viewer finds for a graphics adapter here, and whether its window draws with it.
$code = RunApp @('--hw-report') 'hw-report' 120
Info "--hw-report ended with $code; its output is in hw-report.out.txt"
$shot = Join-Path $Out 'window.png'
$code = RunApp @($samples.jpg, '--size', '1280x800', '--screenshot', $shot) 'window' 180
Info "a screenshot of the window ended with $code; written: $(Test-Path -LiteralPath $shot)"

Write-Host '== Uninstall'
Msiexec @('/x', "`"$Msi`"") (Join-Path $Out 'uninstall.log')
Check 'the program folder is gone' (-not (Test-Path -LiteralPath $dir))
Check 'the file type registration is gone' (-not (Test-Path "$classes\$progId"))
Check 'the Default apps entry is gone' ($null -eq (RegValue 'HKLM:\Software\RegisteredApplications' 'Specular'))
Check 'the Start menu shortcut is gone' (-not (Test-Path -LiteralPath $shortcut))
Check 'the handler of .jpg for all users is still as it was' ((RegValue "$classes\.jpg" '') -eq $jpgBefore)

Write-Host "== Output in $Out"
if ($failed -gt 0) {
    Write-Host "$failed check(s) failed"
    exit 1
}
Write-Host 'All checks passed'
