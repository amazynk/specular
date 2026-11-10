# Runs inside Windows Sandbox (started by run.ps1). Three parts, each line of out\summary.txt
# one PASS/FAIL check (INFO lines report without judging), logs and screenshots beside it:
#   1. The portable zip: runs from an unzipped folder (the decode host too, with no permission
#      change) and leaves nothing in the user's profile or registry.
#   2. The MSI: installs, runs (taskbar id, jump list, close button, file names in other scripts,
#      paths over 260 characters), uninstalls, and nothing of it is left.
# out\done.txt marks the end. The sandbox is a fresh copy of Windows: no Visual C++ runtime, no
# developer tools, no network. This file stays ASCII (Windows PowerShell reads it as ANSI):
# other scripts' characters are built from their code points.

param([switch]$KeepOpen)

$ErrorActionPreference = 'Continue'
$desk = Join-Path $env:USERPROFILE 'Desktop'
$in = Join-Path $desk 'in'
$out = Join-Path $desk 'out'
$lines = [System.Collections.Generic.List[string]]::new()
$appId = 'app.specular.Specular'

function Save { Set-Content -Path (Join-Path $out 'summary.txt') -Value $lines -Encoding utf8 }

function Check([string]$name, [bool]$ok, [string]$detail = '') {
    $mark = if ($ok) { 'PASS' } else { 'FAIL' }
    $line = "$mark  $name"
    if ($detail) { $line += "  ($detail)" }
    $lines.Add($line)
    Save
}

function Info([string]$text) { $lines.Add("INFO  $text"); Save }

# Runs a command-line mode with its output in out\<log>.txt; gives up after $seconds.
function Run([string]$exe, [string[]]$arguments, [string]$log, [int]$seconds = 120) {
    $env:SPECULAR_LOG_FILE = Join-Path $out "$log.log"
    $p = Start-Process -FilePath $exe -ArgumentList $arguments -PassThru -NoNewWindow `
        -RedirectStandardOutput (Join-Path $out "$log.txt") `
        -RedirectStandardError (Join-Path $out "$log.err.txt")
    # Without reading the handle now, PowerShell reports no exit code once the process ended.
    $null = $p.Handle
    if (-not $p.WaitForExit($seconds * 1000)) {
        $p.Kill()
        return 'timeout'
    }
    return $p.ExitCode
}

# A command-line argument in quotes (paths with spaces).
function Q([string]$s) { "`"$s`"" }

# A thumbnail of $file into out\<name>.png: whether it was written.
function Thumb([string]$exe, [string]$file, [string]$name) {
    $png = Join-Path $out "$name.png"
    $code = Run $exe @('--thumbnail', (Q $file), (Q $png), '256') $name
    return (($code -eq 0) -and (Test-Path $png))
}

# A screenshot of the window with $file open: whether it shows more than one colour (a frame
# that failed to render is all black, and a PNG of one colour compresses to almost nothing).
function Shot([string]$exe, [string[]]$arguments, [string]$name) {
    $png = Join-Path $out "$name.png"
    $code = Run $exe ($arguments + @('--size', '1280x800', '--screenshot', (Q $png))) $name
    return (($code -eq 0) -and (Test-Path $png) -and ((Get-Item $png).Length -gt 60000))
}

# Opens a comic book archive in the viewer (a screenshot run): whether its pages were listed.
# RAR and CBR are read by the sandboxed decode host. The desktop thumbnailer (--thumbnail) reads
# plain images only, so archives are checked here.
function Book([string]$exe, [string]$file, [string]$name) {
    $png = Join-Path $out "$name.png"
    $code = Run $exe @((Q $file), '--log', 'folder:debug', '--size', '1280x800', '--screenshot', (Q $png)) $name
    $log = Get-Content (Join-Path $out "$name.log") -Raw -ErrorAction SilentlyContinue
    return (($code -eq 0) -and ($log -match 'archive .*: (ComicInfo, )?[1-9][0-9]* pages'))
}

# Starts the viewer as a user would (a window, its own log and settings), with $file open.
function Open([string]$exe, [string]$file) {
    Remove-Item Env:\SPECULAR_LOG_FILE -ErrorAction SilentlyContinue
    $p = Start-Process -FilePath $exe -ArgumentList @((Q $file)) -PassThru
    $null = $p.Handle
    for ($i = 0; $i -lt 60 -and $p.MainWindowHandle -eq 0 -and -not $p.HasExited; $i++) {
        Start-Sleep -Milliseconds 500
        $p.Refresh()
    }
    return $p
}

# The window's close button (WM_CLOSE, as clicking X sends).
function CloseWindow($p) {
    $p.Refresh()
    $null = $p.CloseMainWindow()
}

# The shortcuts in the Recent items folder that point at $target (their names vary: Explorer
# drops known extensions and numbers clashes).
function RecentLinksTo([string]$target) {
    $wsh = New-Object -ComObject WScript.Shell
    Get-ChildItem $recentDir -Filter '*.lnk' -ErrorAction SilentlyContinue |
        Where-Object { $wsh.CreateShortcut($_.FullName).TargetPath -eq $target } |
        ForEach-Object Name
}

function RegValue([string]$key, [string]$name) {
    try { (Get-ItemProperty -Path $key -Name $name -ErrorAction Stop).$name } catch { $null }
}

function Text([int[]]$codes) { -join ($codes | ForEach-Object { [char]$_ }) }

# Folders and files Specular made in the user's profile (outside the portable folder).
function ProfileTraces {
    $found = @()
    foreach ($base in @($env:LOCALAPPDATA, $env:APPDATA)) {
        $found += Get-ChildItem $base -Directory -Filter 'Specular*' -ErrorAction SilentlyContinue | ForEach-Object FullName
    }
    $found += Get-ChildItem $env:TEMP -Filter '*specular*' -ErrorAction SilentlyContinue | ForEach-Object FullName
    foreach ($key in @('HKCU:\Software\Specular', 'HKCU:\Software\Classes\Specular.Image', 'HKCU:\Software\Classes\specular')) {
        if (Test-Path $key) { $found += $key }
    }
    return $found
}

$msi = Get-ChildItem $in -Filter '*.msi' | Select-Object -First 1
$zip = Get-ChildItem $in -Filter '*.zip' | Select-Object -First 1
$dir = Join-Path $env:ProgramFiles 'Specular'
$exe = Join-Path $dir 'specular.exe'
$shortcut = Join-Path $env:ProgramData 'Microsoft\Windows\Start Menu\Programs\Specular.lnk'
$progId = 'HKLM:\SOFTWARE\Classes\Specular.Image'
$appPaths = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\specular.exe'
$jpgOpenWith = 'HKLM:\SOFTWARE\Classes\.jpg\OpenWithProgids'
$uninstallKeys = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
$recentDir = Join-Path $env:APPDATA 'Microsoft\Windows\Recent'

# Local copies of the samples (the shared "in" folder is read-only and not a normal disk).
$photos = Join-Path $desk 'photos'
New-Item -ItemType Directory $photos -Force | Out-Null
foreach ($s in @('sample.jpg', 'sample.jxl', 'sample.heic', 'sample.cbr')) { Copy-Item (Join-Path $in $s) $photos }
$jpg = Join-Path $photos 'sample.jpg'
$cbr = Join-Path $photos 'sample.cbr'

try {
    $lines.Add("Specular in Windows Sandbox, $(Get-Date -Format s), $([Environment]::OSVersion.VersionString)")
    $vc = Test-Path (Join-Path $env:SystemRoot 'System32\vcruntime140.dll')
    $lines.Add("Visual C++ runtime present in the sandbox: $vc")

    # ===== 1. Portable zip ===================================================================
    $before = ProfileTraces
    Expand-Archive $zip.FullName -DestinationPath (Join-Path $desk 'portable')
    $pdir = Get-ChildItem (Join-Path $desk 'portable') -Directory | Select-Object -First 1 -ExpandProperty FullName
    $pexe = Join-Path $pdir 'specular.exe'
    $shipped = Get-ChildItem $pdir -Recurse -File | ForEach-Object FullName
    Check 'portable: unzipped with the portable marker' ((Test-Path $pexe) -and (Test-Path (Join-Path $pdir 'portable')))
    $code = Run $pexe @('--help') 'portable-help'
    Check 'portable: --help' ($code -eq 0) "exit $code"
    Check 'portable: thumbnail of a JPEG' (Thumb $pexe $jpg 'portable-thumb-jpg')
    Check 'portable: image rendered' (Shot $pexe @((Q $jpg)) 'portable-screen')
    # A session as a user has it: window, settings, recent folders, log; then closed.
    $p = Open $pexe $jpg
    Start-Sleep -Seconds 8
    CloseWindow $p
    Start-Sleep -Seconds 3
    CloseWindow $p
    if (-not $p.WaitForExit(15000)) { $p.Kill() }
    # RAR/CBR are read by the sandboxed decode host, started in an AppContainer from the
    # unzipped folder as it is (no grant to ALL APPLICATION PACKAGES).
    Check 'portable: CBR opens (decode host, no permission change)' (Book $pexe $cbr 'portable-cbr')
    $new = @(ProfileTraces | Where-Object { $before -notcontains $_ })
    $newFiles = $new | Where-Object { Test-Path $_ -PathType Container } |
        ForEach-Object { Get-ChildItem $_ -Recurse -Force -ErrorAction SilentlyContinue | ForEach-Object FullName }
    Check 'portable: nothing written to the user profile or registry' ($new.Count -eq 0) ((@($new) + @($newFiles)) -join ', ')
    $made = Get-ChildItem $pdir -Recurse -File | ForEach-Object FullName | Where-Object { $shipped -notcontains $_ }
    $made = $made | ForEach-Object { $_.Substring($pdir.Length + 1) }
    Info "portable: written inside its folder: $(($made | Select-Object -First 15) -join ', ')$(if (@($made).Count -gt 15) { ', ...' })"
    Get-Process specular, specular-decode-host -ErrorAction SilentlyContinue | Stop-Process -Force

    # ===== 2. MSI: install ===================================================================
    $code = (Start-Process msiexec.exe -Wait -PassThru -ArgumentList @(
        '/i', (Q $msi.FullName), '/qn', '/norestart', '/l*v', (Q (Join-Path $out 'install.log')))).ExitCode
    Check 'install: msiexec exit code 0' ($code -eq 0) "exit $code"
    Check 'install: specular.exe' (Test-Path $exe)
    Check 'install: specular-decode-host.exe' (Test-Path (Join-Path $dir 'specular-decode-host.exe'))
    Check 'install: THIRD-PARTY.md' (Test-Path (Join-Path $dir 'THIRD-PARTY.md'))
    Check 'install: Start menu shortcut' (Test-Path $shortcut)
    $cmd = RegValue "$progId\shell\open\command" '(default)'
    Check 'install: ProgID open command' ($cmd -like "*specular.exe*%1*") "$cmd"
    Check 'install: ProgID taskbar id' ((RegValue $progId 'AppUserModelID') -eq $appId)
    Check 'install: Default apps registration' ((RegValue 'HKLM:\SOFTWARE\RegisteredApplications' 'Specular') -eq 'SOFTWARE\Specular\Capabilities')
    Check 'install: .jpg Open with' ($null -ne (RegValue $jpgOpenWith 'Specular.Image'))
    Check 'install: App Paths' ((RegValue $appPaths '(default)') -like '*specular.exe')
    $arp = Get-ItemProperty $uninstallKeys -ErrorAction SilentlyContinue | Where-Object DisplayName -eq 'Specular'
    Check 'install: Apps & features entry' ($null -ne $arp) "$($arp.DisplayVersion)"

    # ===== 2. MSI: run =======================================================================
    $code = Run $exe @('--help') 'help'
    $text = Get-Content (Join-Path $out 'help.txt') -Raw -ErrorAction SilentlyContinue
    Check 'run: --help (starts without a C++ runtime)' (($code -eq 0) -and ($text -match 'specular')) "exit $code"
    $code = Run $exe @('--hw-report') 'hw-report'
    Check 'run: --hw-report' ($code -eq 0) "exit $code"
    # The sandbox's GPU is software (vGPU off): the D3D12 and Media Foundation probes must find
    # nothing and leave every decode and encode to software.
    $report = Get-Content (Join-Path $out 'hw-report.txt') -Raw -ErrorAction SilentlyContinue
    $line = (($report -split "`n") | Select-String 'Hardware decode') -join ' '
    Check 'run: --hw-report finds no hardware decoder (software GPU)' ($report -match 'Hardware decode\s+(\x1b\[[0-9;]*m)?none') "$line"
    foreach ($s in @('jpg', 'jxl', 'heic')) {
        Check "run: thumbnail of sample.$s" (Thumb $exe (Join-Path $photos "sample.$s") "thumb-$s")
    }
    Check 'run: comic book archive (RAR) opens (decode host)' (Book $exe $cbr 'book-cbr')
    Check 'run: start screen rendered' (Shot $exe @('--show', 'welcome') 'screen-start')
    Check 'run: image rendered' (Shot $exe @((Q $jpg)) 'screen-photo')

    # Taskbar: the window and a pinned shortcut are one button when the process has the
    # shortcut's AppUserModelID; files it views then appear in that button's jump list.
    $shell = New-Object -ComObject Shell.Application
    $lnkId = $shell.Namespace((Split-Path $shortcut)).ParseName('Specular.lnk').ExtendedProperty('System.AppUserModel.ID')
    Check 'taskbar: Start menu shortcut carries the taskbar id' ($lnkId -eq $appId) "$lnkId"
    $code = Run $exe @('--integrate', 'recent-files') 'integrate-recent'
    Check 'recent files: --integrate recent-files' ($code -eq 0) "exit $code"
    $p = Open $exe $jpg
    Start-Sleep -Seconds 10
    Add-Type -Path (Join-Path $in 'jumplist.cs')
    $recent = @([SpecularJumpList]::Recent($appId))
    Check 'taskbar: viewed file in the jump list of the shortcut''s taskbar id' ($recent -contains $jpg) ($recent -join ', ')
    $items = Get-ChildItem $recentDir -File -ErrorAction SilentlyContinue | ForEach-Object Name
    Check 'recent files: shortcut to the file in Recent items' (@(RecentLinksTo $jpg).Count -gt 0) "Recent: $($items -join ', ')"
    Check 'recent files: shortcut to its folder in Recent items' (@(RecentLinksTo $photos).Count -gt 0)
    # The close button with an image open goes back to the start screen; there it quits.
    CloseWindow $p
    Start-Sleep -Seconds 3
    Check 'close button: with an image open, back to the start screen' (-not $p.HasExited)
    CloseWindow $p
    Check 'close button: on the start screen, quits' ($p.WaitForExit(15000))
    if (-not $p.HasExited) { $p.Kill() }
    $code = Run $exe @('--unintegrate', 'recent-files') 'unintegrate-recent'
    Check 'recent files: --unintegrate recent-files' ($code -eq 0) "exit $code"
    $left = @(RecentLinksTo $jpg) + @(RecentLinksTo $photos)
    Check 'recent files: shortcuts to the file and its folder removed again' ($left.Count -eq 0) ($left -join ', ')
    $recent = @([SpecularJumpList]::Recent($appId))
    Check 'recent files: jump list cleared again' ($recent.Count -eq 0) ($recent -join ', ')

    # Single instance: a second start hands its file to the running window and exits; a start
    # without files raises that window; files opened together (one process each, as Explorer
    # does for a selection) end up in one window.
    $viewers = { @(Get-Process specular -ErrorAction SilentlyContinue).Count }
    $p = Open $exe $jpg
    Start-Sleep -Seconds 6
    $jxl = Join-Path $photos 'sample.jxl'
    $second = Start-Process -FilePath $exe -ArgumentList @((Q $jxl)) -PassThru
    $null = $second.Handle
    $handed = $second.WaitForExit(15000)
    Check 'single instance: a second start hands over and exits' ($handed -and $second.ExitCode -eq 0) "exit $(if ($handed) { $second.ExitCode } else { 'none after 15 s' })"
    Start-Sleep -Seconds 2
    Check 'single instance: still one window' ((& $viewers) -eq 1) "$(& $viewers) viewers"
    $null = Run $exe @('--remote', 'state') 'remote-state'
    $state = Get-Content (Join-Path $out 'remote-state.txt') -Raw -ErrorAction SilentlyContinue
    Check 'single instance: the window shows the handed-over file' ($state -match 'sample\.jxl') "$state"
    $third = Start-Process -FilePath $exe -PassThru
    $null = $third.Handle
    $raised = $third.WaitForExit(15000)
    Check 'single instance: a start without files raises the window and exits' ($raised -and $third.ExitCode -eq 0 -and (& $viewers) -eq 1)
    Get-Process specular -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Seconds 2
    $together = foreach ($i in 1..4) {
        $f = Join-Path $photos "together-$i.jpg"
        Copy-Item $jpg $f
        $f
    }
    foreach ($f in $together) { Start-Process -FilePath $exe -ArgumentList @((Q $f)) }
    Start-Sleep -Seconds 15
    Check 'single instance: four files opened at once, one window' ((& $viewers) -eq 1) "$(& $viewers) viewers"
    Get-Process specular -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Seconds 2
    # Started from the Start menu (no file) first, then a file opened from Explorer.
    $null = Start-Process -FilePath $exe -PassThru
    Start-Sleep -Seconds 6
    $later = Start-Process -FilePath $exe -ArgumentList @((Q $jpg)) -PassThru
    $null = $later.Handle
    $handed = $later.WaitForExit(15000)
    Start-Sleep -Seconds 2
    Check 'single instance: a file after a start without files goes to that window' ($handed -and (& $viewers) -eq 1) "$(& $viewers) viewers"
    Get-Process specular -ErrorAction SilentlyContinue | Stop-Process -Force

    # File names in other scripts: German, Chinese and Russian ("Bilder ue photos photo").
    $uml = Text @(0xFC)
    $zh = Text @(0x7167, 0x7247)
    $ru = Text @(0x0444, 0x043E, 0x0442, 0x043E)
    $intl = Join-Path $desk "Bilder $uml $zh $ru"
    New-Item -ItemType Directory $intl -Force | Out-Null
    $names = @{ jpg = "Bild $uml.jpg"; jxl = "$zh.jxl"; heic = "$ru.heic"; cbr = "$ru $zh.cbr" }
    foreach ($s in $names.Keys) {
        $f = Join-Path $intl $names[$s]
        Copy-Item (Join-Path $photos "sample.$s") $f
        if ($s -eq 'cbr') {
            Check 'names: comic book archive in a non-ASCII folder opens' (Book $exe $f 'intl-cbr')
        } else {
            Check "names: thumbnail of a .$s in a non-ASCII folder" (Thumb $exe $f "intl-thumb-$s")
        }
    }
    Check 'names: image rendered from a non-ASCII path' (Shot $exe @((Q (Join-Path $intl $names.jpg))) 'intl-screen')

    # Paths over 260 characters, which need the LongPathsEnabled policy (set here, inside the
    # sandbox only) as well as the manifest's longPathAware.
    $long = Join-Path $desk ('long\' + ((1..6 | ForEach-Object { 'folder-with-a-long-name-' + ('x' * 26) + "-$_" }) -join '\'))
    $null = [IO.Directory]::CreateDirectory("\\?\$long")
    foreach ($s in @('jpg', 'heic', 'cbr')) {
        [IO.File]::Copy((Join-Path $photos "sample.$s"), "\\?\$long\sample.$s")
    }
    $longJpg = "$long\sample.jpg"
    Info "long paths: $($longJpg.Length) characters"
    # Rust's standard library adds the \\?\ prefix to long paths itself, so this may work even
    # without the policy; reported, not judged.
    $nopolicy = Thumb $exe $longJpg 'long-thumb-nopolicy'
    Info "long paths: JPEG thumbnail without LongPathsEnabled: $(if ($nopolicy) { 'opened' } else { 'did not open' })"
    New-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem' -Name LongPathsEnabled -Value 1 -PropertyType DWord -Force | Out-Null
    foreach ($s in @('jpg', 'heic')) {
        Check "long paths: thumbnail of a .$s with LongPathsEnabled" (Thumb $exe "$long\sample.$s" "long-thumb-$s")
    }
    Check 'long paths: comic book archive opens' (Book $exe "$long\sample.cbr" 'long-cbr')
    Check 'long paths: image rendered' (Shot $exe @((Q $longJpg)) 'long-screen')

    # ===== 2. MSI: uninstall =================================================================
    Get-Process specular, specular-decode-host -ErrorAction SilentlyContinue | Stop-Process -Force
    $code = (Start-Process msiexec.exe -Wait -PassThru -ArgumentList @(
        '/x', (Q $msi.FullName), '/qn', '/norestart', '/l*v', (Q (Join-Path $out 'uninstall.log')))).ExitCode
    Check 'uninstall: msiexec exit code 0' ($code -eq 0) "exit $code"
    Check 'uninstall: program folder gone' (-not (Test-Path $dir))
    Check 'uninstall: Start menu shortcut gone' (-not (Test-Path $shortcut))
    Check 'uninstall: ProgID gone' (-not (Test-Path $progId))
    Check 'uninstall: Default apps registration gone' ($null -eq (RegValue 'HKLM:\SOFTWARE\RegisteredApplications' 'Specular'))
    Check 'uninstall: Capabilities key gone' (-not (Test-Path 'HKLM:\SOFTWARE\Specular'))
    Check 'uninstall: .jpg Open with entry gone' ($null -eq (RegValue $jpgOpenWith 'Specular.Image'))
    Check 'uninstall: .jpg keeps its other settings' (Test-Path 'HKLM:\SOFTWARE\Classes\.jpg')
    Check 'uninstall: App Paths gone' (-not (Test-Path $appPaths))
    $arp = Get-ItemProperty $uninstallKeys -ErrorAction SilentlyContinue | Where-Object DisplayName -eq 'Specular'
    Check 'uninstall: Apps & features entry gone' ($null -eq $arp)
    # What running the installed Specular left in the user's profile (settings, caches): kept
    # by design.
    Info "left in the user profile after uninstalling (user data, kept): $((ProfileTraces) -join ', ')"
} catch {
    Check 'harness' $false "$_ at line $($_.InvocationInfo.ScriptLineNumber)"
} finally {
    Save
    Set-Content -Path (Join-Path $out 'done.txt') -Value 'done'
    if (-not $KeepOpen) { shutdown.exe /s /t 5 }
}
