<#
.SYNOPSIS
  Silent install / upgrade / uninstall check of the NSIS installer on a real Windows machine.
  Used by CI. Safe to run locally: it installs into %LocalAppData%\CargoFrame and removes it again.
.PARAMETER Installer
  Path to the *-setup.exe built by `tauri build --bundles nsis`.
#>
param([Parameter(Mandatory = $true)][string]$Installer)

$ErrorActionPreference = 'Stop'
$installDir = Join-Path $env:LOCALAPPDATA 'CargoFrame'
$settingsDir = Join-Path $env:APPDATA 'CargoFrame'
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$uninstKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\CargoFrame'
$failures = @()

function Check([bool]$ok, [string]$what) {
  if ($ok) { Write-Host "ok:   $what" } else { Write-Host "FAIL: $what"; $script:failures += $what }
}
function Run-Installer([string[]]$installerArgs) {
  $p = Start-Process -FilePath $Installer -ArgumentList $installerArgs -Wait -PassThru
  return $p.ExitCode
}
function Get-Exe {
  $name = (Get-ItemProperty -Path $uninstKey -ErrorAction SilentlyContinue).MainBinaryName
  if ($name) { return Join-Path $installDir $name }
  return $null
}
function Running-Count {
  (Get-Process | Where-Object { $_.Path -and $_.Path.StartsWith($installDir, [StringComparison]::OrdinalIgnoreCase) }).Count
}

if (-not (Test-Path $Installer)) { throw "Installer not found: $Installer" }
Write-Host "Installer: $Installer ($([math]::Round((Get-Item $Installer).Length / 1MB, 1)) MB)"

# Clean slate
if (Test-Path $installDir) { Remove-Item $installDir -Recurse -Force }
Remove-ItemProperty -Path $runKey -Name 'CargoFrame' -ErrorAction SilentlyContinue

# 1. Fresh silent install
$code = Run-Installer @('/S')
Check ($code -eq 0) "fresh install exits 0 (got $code)"
$exe = Get-Exe
Check ($null -ne $exe -and (Test-Path $exe)) "program installed in %LocalAppData%\CargoFrame ($exe)"
Check (Test-Path (Join-Path $installDir 'uninstall.exe')) 'uninstall.exe written'
$run = (Get-ItemProperty -Path $runKey -ErrorAction SilentlyContinue).CargoFrame
Check ($run -eq "`"$exe`" --autostart") "run on sign-in is on by default: $run"
$ver = (Get-ItemProperty -Path $uninstKey -ErrorAction SilentlyContinue).DisplayVersion
Check (-not [string]::IsNullOrEmpty($ver)) "listed in Apps & features (version $ver)"
$lnk = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\CargoFrame.lnk'
Check (Test-Path $lnk) 'Start Menu shortcut created'

# 2. Start the app the way the sign-in entry would, and look for it
New-Item -ItemType Directory -Force -Path $settingsDir | Out-Null
Set-Content -Path (Join-Path $settingsDir 'general.json') -Value '{"schema_version":1}'
$proc = Start-Process -FilePath $exe -ArgumentList '--autostart' -PassThru
Start-Sleep -Seconds 6
Check (-not $proc.HasExited) 'app stays running in the background after --autostart'
Check ((Running-Count) -ge 1) 'process found under the install folder'

# 2b. What the always-on process costs while it sits in the tray (performance plan: under 40 MB,
# effectively 0% CPU). The numbers are reported; only a busy loop fails here, because the memory
# budget still has to be confirmed on a reference machine (docs/performance.md).
$proc.Refresh()
$cpu0 = $proc.TotalProcessorTime.TotalSeconds
$wall = 10
Start-Sleep -Seconds $wall
$proc.Refresh()
$cpuPct = [math]::Round((($proc.TotalProcessorTime.TotalSeconds - $cpu0) / $wall / [Environment]::ProcessorCount) * 100, 2)
$privMb = [math]::Round($proc.PrivateMemorySize64 / 1MB, 1)
$wsMb = [math]::Round($proc.WorkingSet64 / 1MB, 1)
Write-Host "tray-only idle: $privMb MB private, $wsMb MB working set, $cpuPct% CPU over ${wall}s, $($proc.Threads.Count) threads"
if ($env:GITHUB_STEP_SUMMARY) {
  Add-Content -Path $env:GITHUB_STEP_SUMMARY -Value @(
    '### Tray-only idle (Windows)',
    "- Private memory: $privMb MB (budget 40 MB; report only for now)",
    "- Working set: $wsMb MB",
    "- CPU: $cpuPct% over ${wall}s (gate 2%)"
  )
}
Check ($cpuPct -le 2) "idle CPU is effectively zero ($cpuPct%)"

# 3. Upgrade in place while it runs (same version counts as an upgrade here)
$code = Run-Installer @('/S')
Check ($code -eq 0) "upgrade over a running copy exits 0 (got $code)"
Check ((Test-Path $exe)) 'program present after upgrade'
Check ((Running-Count) -eq 0) 'running copy was asked to quit before files were replaced'
Check (Test-Path (Join-Path $settingsDir 'general.json')) 'settings kept by the upgrade'
$run = (Get-ItemProperty -Path $runKey -ErrorAction SilentlyContinue).CargoFrame
Check ($run -eq "`"$exe`" --autostart") 'run on sign-in kept by the upgrade'

# 4. An upgrade keeps a person's choice to not run on sign-in
Remove-ItemProperty -Path $runKey -Name 'CargoFrame' -ErrorAction SilentlyContinue
$code = Run-Installer @('/S')
Check ($code -eq 0) 'upgrade with startup off exits 0'
$run = (Get-ItemProperty -Path $runKey -ErrorAction SilentlyContinue).CargoFrame
Check ([string]::IsNullOrEmpty($run)) 'run on sign-in stays off after an upgrade'

# 5. Uninstall (in place, so we can wait for it)
$code = (Start-Process -FilePath (Join-Path $installDir 'uninstall.exe') -ArgumentList @('/S', "_?=$installDir") -Wait -PassThru).ExitCode
Check ($code -eq 0) "uninstall exits 0 (got $code)"
Check (-not (Test-Path $exe)) 'program removed'
Check (-not (Test-Path $uninstKey)) 'removed from Apps & features'
Check (-not (Test-Path $lnk)) 'Start Menu shortcut removed'
Check (Test-Path (Join-Path $settingsDir 'general.json')) 'settings kept by default on uninstall'
$run = (Get-ItemProperty -Path $runKey -ErrorAction SilentlyContinue).CargoFrame
Check ([string]::IsNullOrEmpty($run)) 'run on sign-in entry removed'

# 6. Fresh install with /NOSTARTUP
if (Test-Path $installDir) { Remove-Item $installDir -Recurse -Force -ErrorAction SilentlyContinue }
$code = Run-Installer @('/S', '/NOSTARTUP')
Check ($code -eq 0) 'install with /NOSTARTUP exits 0'
$run = (Get-ItemProperty -Path $runKey -ErrorAction SilentlyContinue).CargoFrame
Check ([string]::IsNullOrEmpty($run)) '/NOSTARTUP leaves run on sign-in off'
Start-Process -FilePath (Join-Path $installDir 'uninstall.exe') -ArgumentList @('/S', "_?=$installDir") -Wait | Out-Null
Remove-Item $settingsDir -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item $installDir -Recurse -Force -ErrorAction SilentlyContinue

if ($failures.Count -gt 0) {
  Write-Host "`n$($failures.Count) check(s) failed:"
  $failures | ForEach-Object { Write-Host " - $_" }
  exit 1
}
Write-Host "`nAll installer checks passed."
