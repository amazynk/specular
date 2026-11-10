# Tests the portable zip and the MSI in Windows Sandbox: a throw-away copy of this Windows with
# nothing installed. Runs the portable copy, then installs, checks, runs and uninstalls the MSI
# there (inside.ps1), unattended, and prints the results. The sandbox has no network and closes itself when done (-KeepOpen to look around).
#
#   powershell -ExecutionPolicy Bypass -File packaging\windows\sandbox\run.ps1
#
# Needs the Windows Sandbox feature (Windows 10/11 Pro or Enterprise), once, as administrator:
#   Enable-WindowsOptionalFeature -Online -FeatureName Containers-DisposableClientVM -All -NoRestart
# then restart Windows when it suits you (without -NoRestart the command restarts it right
# away). Results: target\sandbox\out (summary.txt, msiexec logs, screenshots).

param(
    [string]$Msi = '',
    [int]$TimeoutMinutes = 20,
    [switch]$KeepOpen,
    # No GPU in the sandbox (vGPU sharing crashes the sandbox on some systems); the screenshots
    # then need software rendering and may fail, the other checks do not need a GPU.
    [switch]$NoGpu
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$sandbox = Join-Path $env:SystemRoot 'System32\WindowsSandbox.exe'
if (-not (Test-Path $sandbox)) {
    throw 'Windows Sandbox is not enabled. As administrator: Enable-WindowsOptionalFeature -Online -FeatureName Containers-DisposableClientVM -All -NoRestart, then restart Windows when it suits you.'
}
if (-not $Msi) {
    $Msi = Get-ChildItem (Join-Path $root 'target\dist') -Recurse -Filter 'Specular-*-x64.msi' |
        Sort-Object LastWriteTime | Select-Object -Last 1 -ExpandProperty FullName
}
if (-not $Msi -or -not (Test-Path $Msi)) { throw 'No MSI: build one with ./build.sh release windows --package' }

# The shared folders live in a plain folder under %TEMP% rather than under the source tree,
# which may sit behind a mount point or junction (a guess at one cause of sandbox start
# failures); the results are copied to target\sandbox\out at the end.
$work = Join-Path $env:TEMP 'specular-sandbox'
$in = Join-Path $work 'in'
$out = Join-Path $work 'out'
$results = Join-Path $root 'target\sandbox\out'
foreach ($d in @($in, $out, $results)) {
    if (Test-Path $d) { Remove-Item $d -Recurse -Force }
    New-Item -ItemType Directory $d | Out-Null
}
$zip = Join-Path (Split-Path $Msi) "Specular-$((Split-Path $Msi -Leaf) -replace '^Specular-(.*)-x64\.msi$','$1')-windows-x64.zip"
if (-not (Test-Path $zip)) { throw "No portable zip next to the MSI: $zip" }
Copy-Item $Msi $in
Copy-Item $zip $in
Copy-Item (Join-Path $PSScriptRoot 'inside.ps1') $in
Copy-Item (Join-Path $PSScriptRoot 'jumplist.cs') $in
# One file per decoder path: JPEG (in-process), JPEG XL (libjxl), HEIC, and a comic book RAR
# (the sandboxed decode host; its image is sub/grad.png).
Copy-Item (Join-Path $root 'crates\formats\specular-fmt-thermal\tests\data\flir_noraw.jpg') (Join-Path $in 'sample.jpg')
Copy-Item (Join-Path $root 'crates\formats\specular-jxl\tests\data\alpha.jxl') (Join-Path $in 'sample.jxl')
Copy-Item (Join-Path $root 'crates\formats\specular-heic\fuzz\seeds\c420.heic') (Join-Path $in 'sample.heic')
Copy-Item (Join-Path $root 'crates\specular-decode-host\tests\data\rar5.rar') (Join-Path $in 'sample.cbr')

$box = 'C:\Users\WDAGUtilityAccount\Desktop'
$keep = if ($KeepOpen) { ' -KeepOpen' } else { '' }
$gpu = if ($NoGpu) { 'Disable' } else { 'Enable' }
$wsb = @"
<Configuration>
  <Networking>Disable</Networking>
  <vGPU>$gpu</vGPU>
  <ClipboardRedirection>Disable</ClipboardRedirection>
  <MappedFolders>
    <MappedFolder><HostFolder>$in</HostFolder><SandboxFolder>$box\in</SandboxFolder><ReadOnly>true</ReadOnly></MappedFolder>
    <MappedFolder><HostFolder>$out</HostFolder><SandboxFolder>$box\out</SandboxFolder><ReadOnly>false</ReadOnly></MappedFolder>
  </MappedFolders>
  <LogonCommand>
    <Command>powershell.exe -NoProfile -ExecutionPolicy Bypass -File $box\in\inside.ps1$keep</Command>
  </LogonCommand>
</Configuration>
"@
$config = Join-Path $work 'specular.wsb'
Set-Content -Path $config -Value $wsb -Encoding utf8

Write-Host "Testing $(Split-Path $Msi -Leaf) in Windows Sandbox (vGPU $gpu; results in $results)..."
Start-Process $sandbox -ArgumentList "`"$config`""
$deadline = (Get-Date).AddMinutes($TimeoutMinutes)
while (-not (Test-Path (Join-Path $out 'done.txt'))) {
    if ((Get-Date) -gt $deadline) {
        Copy-Item (Join-Path $out '*') $results -Recurse -ErrorAction SilentlyContinue
        throw "No result after $TimeoutMinutes minutes (did the sandbox start? see its window); partial results in $results"
    }
    Start-Sleep -Seconds 5
}
Copy-Item (Join-Path $out '*') $results -Recurse
$summary = Get-Content (Join-Path $results 'summary.txt')
$summary | Write-Host
if ($summary -match '^FAIL') { exit 1 }
