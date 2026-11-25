param(
    [Parameter(Mandatory)]
    [ValidateSet('smoke', 'release-gate')]
    [string]$Mode,
    [Parameter(Mandatory)]
    [string]$OutputDirectory,
    [string]$Toolchain = '1.98.0'
)

$ErrorActionPreference = 'Stop'
$project = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).ProviderPath
$out = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $out) { throw 'Choose a new directory for benchmark evidence.' }
New-Item -ItemType Directory -Path $out | Out-Null
$out = (Resolve-Path -LiteralPath $out).ProviderPath
$drive = [IO.DriveInfo]::new([IO.Path]::GetPathRoot($out))
if ($drive.DriveType -ne [IO.DriveType]::Fixed -or $drive.DriveFormat -ne 'NTFS') {
    throw 'The Windows reference run requires a fixed local NTFS output volume.'
}

$exe = Join-Path $project 'target/debug/examples/performance_soak.exe'
$session = Join-Path $out 'synthetic-session.sqlite'
$results = Join-Path $out 'phases.log'
$errors = Join-Path $out 'child-errors.log'
$memory = Join-Path $out 'memory.csv'
$sampleEvery = if ($Mode -eq 'smoke') { 1 } else { 15 }
$warmup = if ($Mode -eq 'smoke') { 2 } else { 300 }
$minimumSamples = if ($Mode -eq 'smoke') { 8 } else { 400 }
$deadlineSeconds = if ($Mode -eq 'smoke') { 60 } else { 7500 }
$maxPrivate = 1GB
$maxGrowth = 256MB

Push-Location -LiteralPath $project
try {
    $base = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Git source revision is unavailable.' }
    if ($Mode -eq 'release-gate' -and (& git status --porcelain)) {
        throw 'The release performance gate requires a clean source checkout.'
    }
    & cargo "+$Toolchain" build -p exmon-gui --example performance_soak --locked
    if ($LASTEXITCODE -ne 0) { throw 'Benchmark build failed.' }
    # Reuse the recorder's deterministic bounded write-worker stall fixture.
    & cargo "+$Toolchain" test -p exmon-app --lib --locked sessions::tests::full_data_queue_stop_and_stale_token_are_independent_of_store_progress -- --exact
    if ($LASTEXITCODE -ne 0) { throw 'Storage backpressure fixture failed.' }
    $exeHash = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
    $cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
    $hostRam = (Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory
    $os = Get-CimInstance Win32_OperatingSystem
    $elapsed = [Diagnostics.Stopwatch]::StartNew()
    $child = Start-Process -FilePath $exe -ArgumentList "--$Mode", ('"' + $session + '"') -WindowStyle Hidden -PassThru -RedirectStandardOutput $results -RedirectStandardError $errors
    # Windows PowerShell can otherwise lose the exit code after process exit.
    $null = $child.Handle
    $samples = 0
    $baseline = $null
    $peakPrivate = 0L
    $peakWorkingSet = 0L
    $postWarmupPeak = 0L
    'elapsed_ms,private_bytes,working_set_bytes' | Set-Content -LiteralPath $memory -Encoding Ascii
    try {
        while (!$child.WaitForExit(1000 * $sampleEvery)) {
            if ($elapsed.Elapsed.TotalSeconds -ge $deadlineSeconds) { throw 'Benchmark exceeded the supervised deadline.' }
            $child.Refresh()
            if ($child.HasExited) { break }
            try {
                $private = $child.PrivateMemorySize64
                $working = $child.WorkingSet64
            } catch [InvalidOperationException] {
                if ($child.HasExited) { break }
                throw
            }
            $samples++
            $peakPrivate = [Math]::Max($peakPrivate, $private)
            $peakWorkingSet = [Math]::Max($peakWorkingSet, $working)
            if ($elapsed.Elapsed.TotalSeconds -ge $warmup) {
                if ($null -eq $baseline) { $baseline = $private }
                $postWarmupPeak = [Math]::Max($postWarmupPeak, $private)
            }
            ('{0},{1},{2}' -f $elapsed.ElapsedMilliseconds, $private, $working) | Add-Content -LiteralPath $memory -Encoding Ascii
            if ($peakPrivate -gt $maxPrivate) { throw 'Benchmark process exceeded its private-memory ceiling.' }
        }
        $child.WaitForExit()
    } finally {
        if (!$child.HasExited) {
            Stop-Process -InputObject $child
            $child.WaitForExit()
        }
    }
    $lines = [IO.File]::ReadAllLines($results)
    $phases = @($lines | Where-Object { $_ -match '^phase=' })
    $coverage = $phases.Count -eq 3 -and
        $phases[0] -match '^phase=small_packets .+ pass=true$' -and
        $phases[1] -match '^phase=mixed_fragmented .+ pass=true$' -and
        $phases[2] -match '^phase=chart_heavy .+ pass=true$'
    $growth = if ($null -ne $baseline) { $postWarmupPeak - $baseline } else { $null }
    $passed = $child.ExitCode -eq 0 -and $coverage -and
        ($lines -contains "workloads=3 mode=--$Mode verdict=pass") -and
        $samples -ge $minimumSamples -and $null -ne $growth -and $growth -le $maxGrowth
    [ordered]@{
        mode = $Mode
        source_commit = $base
        executable_sha256 = $exeHash
        toolchain = $Toolchain
        cpu = $cpu.Name
        logical_cpus = $cpu.NumberOfLogicalProcessors
        host_memory_bytes = $hostRam
        os = $os.Caption
        os_build = $os.BuildNumber
        filesystem = $drive.DriveFormat
        elapsed_seconds = [Math]::Round($elapsed.Elapsed.TotalSeconds, 3)
        sampling_interval_seconds = $sampleEvery
        samples = $samples
        peak_private_bytes = $peakPrivate
        peak_working_set_bytes = $peakWorkingSet
        private_growth_bytes = $growth
        slow_storage_fixture = 'passed'
        phase_results = $phases
        exit_code = $child.ExitCode
        passed = $passed
    } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $out 'summary.json') -Encoding UTF8
    $phases | Write-Output
    if (!$passed) { throw 'Performance verdict failed. See summary.json and child logs.' }
    Write-Output "PASS $Mode"
} finally {
    Pop-Location
}
