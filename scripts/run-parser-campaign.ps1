param([Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$project = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).ProviderPath
$out = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $out) { throw 'Choose a new output directory.' }
New-Item -ItemType Directory -Path $out | Out-Null
Push-Location -LiteralPath $project
try {
    $manifest = & cargo +1.98.0 test -p exmon-app --test parser_campaign --locked --no-run --message-format=json
    if ($LASTEXITCODE -ne 0) { throw 'Campaign build failed.' }
    $exe = @($manifest | ForEach-Object { $_ | ConvertFrom-Json } | Where-Object {
        $_.reason -eq 'compiler-artifact' -and $_.target.name -eq 'parser_campaign' -and $_.executable
    })[-1].executable
    if (!$exe) { throw 'Campaign executable missing.' }
    $child = Start-Process -FilePath $exe -ArgumentList 'mutation_campaign','--ignored','--exact','--nocapture' -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $out 'campaign.log') -RedirectStandardError (Join-Path $out 'errors.log')
    $null = $child.Handle
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $peak = 0L
    try {
        while (!$child.WaitForExit(500)) {
            $child.Refresh()
            if ($child.HasExited) { break }
            $peak = [Math]::Max($peak, $child.PrivateMemorySize64)
            if ($clock.Elapsed.TotalSeconds -gt 100 -or $child.TotalProcessorTime.TotalSeconds -gt 90 -or $peak -gt 256MB) {
                throw 'Campaign resource limit exceeded.'
            }
        }
        $child.WaitForExit()
    } finally {
        if (!$child.HasExited) { Stop-Process -InputObject $child; $child.WaitForExit() }
    }
    $lines = [IO.File]::ReadAllLines((Join-Path $out 'campaign.log'))
    $families = @($lines | Where-Object { $_ -match '^family=(network|capture|framing|codec) seconds=.+ cases=\d+ pass=true$' })
    $passed = $child.ExitCode -eq 0 -and $families.Count -eq 4
    [ordered]@{
        source = (& git rev-parse HEAD).Trim()
        executable_sha256 = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
        elapsed_seconds = $clock.Elapsed.TotalSeconds
        cpu_seconds = $child.TotalProcessorTime.TotalSeconds
        peak_private_bytes = $peak
        families = $families
        passed = $passed
        exit_code = $child.ExitCode
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $out 'summary.json') -Encoding UTF8
    $families | Write-Output
    if (!$passed) { throw 'Campaign failed. Reproduce the family and case in errors.log.' }
} finally { Pop-Location }
