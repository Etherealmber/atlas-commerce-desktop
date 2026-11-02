# Exercise the actual release GUI, not only --version or cargo test. The
# isolated profile keeps this test from reading credentials or an existing
# Atlas instance's settings on the runner.
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class AtlasWindowSmoke {
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindowAsync(IntPtr hWnd, int command);
}
'@

$exe = (Resolve-Path 'target/release/atlas-commerce-desktop.exe').Path
$profile = Join-Path $env:RUNNER_TEMP ('atlas-launch-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $profile | Out-Null
$oldAppData = $env:APPDATA
$oldLocalAppData = $env:LOCALAPPDATA
$env:APPDATA = Join-Path $profile 'roaming'
$env:LOCALAPPDATA = Join-Path $profile 'local'
New-Item -ItemType Directory -Path $env:APPDATA, $env:LOCALAPPDATA | Out-Null
$process = $null
$second = $null
try {
    $stderr = Join-Path $profile 'stderr.txt'
    $stdout = Join-Path $profile 'stdout.txt'
    $process = Start-Process -FilePath $exe -PassThru -RedirectStandardError $stderr -RedirectStandardOutput $stdout
    $deadline = [DateTime]::UtcNow.AddSeconds(60)
    $handle = [IntPtr]::Zero
    while ([DateTime]::UtcNow -lt $deadline) {
        $process.Refresh()
        if ($process.HasExited) { throw "App exited before opening its window (exit $($process.ExitCode))." }
        $handle = $process.MainWindowHandle
        if ($handle -ne [IntPtr]::Zero -and [AtlasWindowSmoke]::IsWindowVisible($handle)) { break }
        Start-Sleep -Milliseconds 500
    }
    if ($handle -eq [IntPtr]::Zero -or -not [AtlasWindowSmoke]::IsWindowVisible($handle)) {
        throw 'The app is running but did not open a visible main window.'
    }
    Write-Host "Visible window opened (PID $($process.Id))."

    # A second launch must restore an existing minimized instance rather than
    # silently exiting while the only window remains hidden in the taskbar.
    [void][AtlasWindowSmoke]::ShowWindowAsync($handle, 6) # SW_MINIMIZE
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    while (-not [AtlasWindowSmoke]::IsIconic($handle) -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 200
    }
    if (-not [AtlasWindowSmoke]::IsIconic($handle)) { throw 'Could not minimize the window for the restore test.' }
    $second = Start-Process -FilePath $exe -PassThru
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    while ([DateTime]::UtcNow -lt $deadline) {
        $process.Refresh()
        if ($process.HasExited) { throw 'The original app exited during the restore test.' }
        if (-not [AtlasWindowSmoke]::IsIconic($handle) -and [AtlasWindowSmoke]::IsWindowVisible($handle)) { break }
        Start-Sleep -Milliseconds 500
    }
    if ([AtlasWindowSmoke]::IsIconic($handle) -or -not [AtlasWindowSmoke]::IsWindowVisible($handle)) {
        throw 'A second launch did not restore the minimized window.'
    }
    Write-Host 'Second launch restored the minimized window.'
} catch {
    $detail = $_.Exception.Message -replace '[\r\n]', ' '
    Write-Host "::error title=Windows launch smoke::$detail"
    # This profile is new and contains no real user data or credentials.
    foreach ($output in @('stdout.txt', 'stderr.txt')) {
        $outputPath = Join-Path $profile $output
        if (Test-Path $outputPath) {
            Get-Content $outputPath -Tail 20 | ForEach-Object {
                $line = $_ -replace '[\r\n]', ' '
                Write-Host "::warning title=Windows process output::$line"
            }
        }
    }
    Get-ChildItem -Path $profile -Filter '*.log' -Recurse -ErrorAction SilentlyContinue |
        ForEach-Object {
            Write-Host "--- $($_.Name) ---"
            Get-Content $_.FullName -Tail 20 | ForEach-Object {
                $line = $_ -replace '[\r\n]', ' '
                Write-Host "::warning title=Fresh-profile launch log::$line"
            }
        }
    throw
} finally {
    foreach ($child in @($second, $process)) {
        if ($null -ne $child) {
            $child.Refresh()
            if (-not $child.HasExited) { Stop-Process -Id $child.Id -Force -ErrorAction SilentlyContinue }
        }
    }
    $env:APPDATA = $oldAppData
    $env:LOCALAPPDATA = $oldLocalAppData
    Remove-Item -LiteralPath $profile -Recurse -Force -ErrorAction SilentlyContinue
}
