# Run only on an ephemeral GitHub-hosted Windows runner. Exercises two real
# packages in separate temporary directories, then removes the MSI and repairs
# Inno's shared registrations. It does not automate migration on user PCs.
param(
    [Parameter(Mandatory = $true)][string]$Msi,
    [Parameter(Mandatory = $true)][string]$InnoSetup
)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'This destructive packaging test requires a disposable GitHub-hosted runner.'
}
$roots = @('HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall',
    'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall',
    'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall')
$existing = @(foreach ($root in $roots) {
    Get-ItemProperty "$root\*" -ErrorAction SilentlyContinue |
        Where-Object { $_.DisplayName -in @('Atlas Commerce', 'Atlas Commerce Desktop') }
})
if ($existing.Count -ne 0) { throw 'Existing Atlas installation detected; refusing to run.' }
$work = Join-Path $env:RUNNER_TEMP ('atlas-mixed-' + [guid]::NewGuid().ToString('N'))
$msiDir = Join-Path $work 'msi'
$innoDir = Join-Path $work 'inno'
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{E74305A7-4960-45B6-93C9-CEA8D83B1517}_is1'
$msiInstalled = $false
New-Item -ItemType Directory -Force $work | Out-Null
try {
    $args = @('/i', "`"$Msi`"", '/qn', '/norestart', "INSTALLFOLDER=`"$msiDir`"")
    $p = Start-Process msiexec.exe -ArgumentList $args -Wait -PassThru
    if ($p.ExitCode -ne 0) { throw "MSI install failed: $($p.ExitCode)" }
    $msiInstalled = $true
    if (-not (Test-Path (Join-Path $msiDir 'atlas-commerce-desktop.exe'))) { throw 'MSI binary missing' }
    $args = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-', "/DIR=`"$innoDir`"")
    $p = Start-Process -FilePath $InnoSetup -ArgumentList $args -Wait -PassThru
    if ($p.ExitCode -ne 0) { throw "Inno install failed: $($p.ExitCode)" }
    if (-not (Test-Path $key) -or -not (Test-Path (Join-Path $innoDir 'atlas-commerce-desktop.exe'))) { throw 'Inno install missing' }
    if (-not (Test-Path (Join-Path $msiDir 'atlas-commerce-desktop.exe'))) { throw 'MSI copy disappeared' }
    $p = Start-Process msiexec.exe -ArgumentList @('/x', "`"$Msi`"", '/qn', '/norestart') -Wait -PassThru
    if ($p.ExitCode -ne 0) { throw "MSI uninstall failed: $($p.ExitCode)" }
    $msiInstalled = $false
    if (-not (Test-Path (Join-Path $innoDir 'atlas-commerce-desktop.exe'))) { throw 'MSI uninstall removed Inno binary' }
    # MSI and Inno both own the protocol: re-run Inno to restore its entries.
    $p = Start-Process -FilePath $InnoSetup -ArgumentList $args -Wait -PassThru
    if ($p.ExitCode -ne 0) { throw "Inno repair failed: $($p.ExitCode)" }
    $command = (Get-ItemProperty 'HKCU:\Software\Classes\atlascommerce\shell\open\command').'(default)'
    if (-not $command.Contains((Join-Path $innoDir 'atlas-commerce-desktop.exe'))) { throw 'Protocol not restored to Inno copy' }
    if (Test-Path (Join-Path $msiDir 'atlas-commerce-desktop.exe')) { throw 'MSI binary remains after uninstall' }
    Write-Output 'Mixed MSI/Inno test: separate installs, remove MSI, repair Inno, protocol restored.'
}
finally {
    if ($msiInstalled) {
        $p = Start-Process msiexec.exe -ArgumentList @('/x', "`"$Msi`"", '/qn', '/norestart') -Wait -PassThru
        if ($p.ExitCode -ne 0) { Write-Warning "Cleanup MSI failed: $($p.ExitCode)" }
    }
    $uninstaller = Join-Path $innoDir 'unins000.exe'
    if (Test-Path $uninstaller) {
        $p = Start-Process -FilePath $uninstaller -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -Wait -PassThru
        if ($p.ExitCode -ne 0) { Write-Warning "Cleanup Inno failed: $($p.ExitCode)" }
    }
    if (Test-Path $work) { Remove-Item -Recurse -Force $work }
}
