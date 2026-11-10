# Exercise two real silent Inno installations on a disposable CI runner.
# This does not test the GitHub updater handoff or migrate existing MSI copies.
param(
    [Parameter(Mandatory = $true)][string]$Compiler,
    [Parameter(Mandatory = $true)][string]$Binary,
    [Parameter(Mandatory = $true)][string]$CurrentSetup
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$work = Join-Path $env:RUNNER_TEMP ('atlas-upgrade-' + [guid]::NewGuid().ToString('N'))
$install = Join-Path $work 'installed'
$build = Join-Path $work 'old-setup'
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{E74305A7-4960-45B6-93C9-CEA8D83B1517}_is1'
if (Test-Path $key) { throw 'Existing Atlas Inno install found; refusing to touch it.' }
New-Item -ItemType Directory -Force $build | Out-Null
try {
    & $Compiler '/DVersion=0.0.1' "/DBinaryX64=$Binary" "/DBinaryArm64=$Binary" "/DOutputDir=$build" (Join-Path $repo 'packaging\windows\atlas-commerce-desktop.iss')
    if ($LASTEXITCODE -ne 0) { throw "Old setup build failed: $LASTEXITCODE" }
    $old = Join-Path $build 'atlas-commerce-desktop-v0.0.1-setup.exe'
    $first = $true
    foreach ($setup in @($old, $CurrentSetup)) {
        $setupArgs = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-', "/DIR=`"$install`"")
        if ($first) { $setupArgs += '/SERVERURL=https://shop.example.invalid' }
        $process = Start-Process -FilePath $setup -ArgumentList $setupArgs -Wait -PassThru
        if ($process.ExitCode -ne 0) { throw "Setup failed: $($process.ExitCode)" }
        if (-not (Test-Path (Join-Path $install 'atlas-commerce-desktop.exe'))) { throw 'Installed executable missing' }
        $choiceFile = Join-Path $install 'server.json'
        if (-not (Test-Path $choiceFile)) { throw 'Installer did not provision a server' }
        $choice = Get-Content -Raw $choiceFile | ConvertFrom-Json
        if ($choice.url -ne 'https://shop.example.invalid') { throw "Server was changed by the upgrade: $($choice.url)" }
        $first = $false
        if (-not (Test-Path $key)) { throw 'Stable Inno uninstall registration missing' }
        $entry = Get-ItemProperty $key
        if ($entry.DisplayName -ne 'Atlas Commerce') { throw "Unexpected display name: $($entry.DisplayName)" }
        if ($entry.InstallLocation.TrimEnd('\') -ne $install.TrimEnd('\')) { throw 'Install location changed' }
    }
    $entries = @(Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' | Where-Object { $_.PSChildName -like '*E74305A7-4960-45B6-93C9-CEA8D83B1517*' })
    if ($entries.Count -ne 1) { throw "Expected one Inno uninstall registration, found $($entries.Count)" }
    Write-Output 'Inno upgrade: two real silent installs, one registration, same executable path.'
}
finally {
    $uninstaller = Join-Path $install 'unins000.exe'
    if (Test-Path $uninstaller) {
        $process = Start-Process -FilePath $uninstaller -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -Wait -PassThru
        if ($process.ExitCode -ne 0) { throw "Test uninstall failed: $($process.ExitCode)" }
    }
    if (Test-Path $work) { Remove-Item -Recurse -Force $work }
}
