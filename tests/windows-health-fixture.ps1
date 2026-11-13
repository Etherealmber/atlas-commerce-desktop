# Disposable local endpoint; never accepts real credentials or contacts a server.
function Start-AtlasHealthFixture([string]$Work) {
    $portFile = Join-Path $Work 'health-port.txt'
    $script = Join-Path $PSScriptRoot 'installer-health-server.py'
    $process = Start-Process python.exe -ArgumentList @("`"$script`"", "`"$portFile`"") -PassThru -WindowStyle Hidden
    for ($i = 0; $i -lt 100 -and -not (Test-Path $portFile); $i++) {
        if ($process.HasExited) { throw 'Health fixture exited before starting.' }
        Start-Sleep -Milliseconds 100
    }
    if (-not (Test-Path $portFile)) { Stop-Process -Id $process.Id -ErrorAction SilentlyContinue; throw 'Health fixture did not start.' }
    $port = (Get-Content -Raw $portFile).Trim()
    return @{ Process = $process; Url = "http://127.0.0.1:$port" }
}
