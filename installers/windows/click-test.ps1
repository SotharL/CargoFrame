<#
.SYNOPSIS
  Runs the installer the way a person does: opens its window, presses the one button, waits for it
  to close by itself, and checks that CargoFrame was installed and opened. Used by CI after the
  silent checks (smoke-test.ps1). Screenshots go to -Shots so a failure can be looked at.
  Safe to run locally: it installs into %LocalAppData%\CargoFrame and removes it again.
.PARAMETER Installer
  Path to the *-setup.exe.
.PARAMETER Shots
  Folder for the screenshots (created if missing).
#>
param(
  [Parameter(Mandatory = $true)][string]$Installer,
  [string]$Shots = (Join-Path ([IO.Path]::GetTempPath()) 'cargoframe-click-test')
)

$ErrorActionPreference = 'Stop'
$installDir = Join-Path $env:LOCALAPPDATA 'CargoFrame'
$settingsDir = Join-Path $env:APPDATA 'CargoFrame'
$uninstKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\CargoFrame'
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$failures = @()
New-Item -ItemType Directory -Force -Path $Shots | Out-Null

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class Win {
  [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr hWnd, int id);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hWnd, uint msg, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
  public static string Text(IntPtr h) { var sb = new StringBuilder(256); GetWindowText(h, sb, 256); return sb.ToString(); }
}
'@

function Check([bool]$ok, [string]$what) {
  if ($ok) { Write-Host "ok:   $what" } else { Write-Host "FAIL: $what"; $script:failures += $what }
}
function Shot([string]$name) {
  try {
    $b = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($b.Left, $b.Top, 0, 0, $bmp.Size)
    $bmp.Save((Join-Path $Shots "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
  } catch { Write-Host "(no screenshot: $($_.Exception.Message))" }
}
function Wait-For([scriptblock]$cond, [int]$seconds) {
  $end = (Get-Date).AddSeconds($seconds)
  while ((Get-Date) -lt $end) { if (& $cond) { return $true }; Start-Sleep -Milliseconds 400 }
  return $false
}
function Installed-Procs {
  Get-Process | Where-Object { $_.Path -and $_.Path.StartsWith($installDir, [StringComparison]::OrdinalIgnoreCase) }
}

if (-not (Test-Path $Installer)) { throw "Installer not found: $Installer" }

# Clean slate
if (Test-Path $installDir) { Remove-Item $installDir -Recurse -Force -ErrorAction SilentlyContinue }
Remove-Item $settingsDir -Recurse -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $runKey -Name 'CargoFrame' -ErrorAction SilentlyContinue

# 1. Open the installer window, as a double-click does
$inst = Start-Process -FilePath $Installer -PassThru
$shown = Wait-For { $inst.Refresh(); $inst.MainWindowHandle -ne [IntPtr]::Zero } 60
Check $shown 'the installer opens a window'
if ($shown) {
  Start-Sleep -Seconds 1
  Shot 'installer-1-start'
  $inst.Refresh()
  Write-Host "window title: $($inst.MainWindowTitle)"
  $button = [Win]::GetDlgItem($inst.MainWindowHandle, 1)
  $label = if ($button -ne [IntPtr]::Zero) { [Win]::Text($button) } else { '' }
  Write-Host "button: '$label'"
  Check ($label -eq 'Install') "the one button says Install (it says '$label')"
  Check ([Win]::IsWindowVisible($button)) 'the button is visible'
  # Nothing is installed until the button is pressed
  Check (-not (Test-Path (Join-Path $installDir 'cargoframe.exe'))) 'nothing is installed before the button is pressed'

  # 2. Press it
  [void][Win]::PostMessage($button, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero)   # BM_CLICK
  Start-Sleep -Seconds 2
  Shot 'installer-2-working'
  $closed = Wait-For { $inst.Refresh(); $inst.HasExited } 120
  Check $closed 'the installer closes by itself when it is done'
  if (-not $closed) { Shot 'installer-3-stuck'; Stop-Process -Id $inst.Id -Force -ErrorAction SilentlyContinue }
}

# 3. What a person gets
$exe = Join-Path $installDir 'cargoframe.exe'
if (-not (Test-Path $exe)) {
  $name = (Get-ItemProperty -Path $uninstKey -ErrorAction SilentlyContinue).MainBinaryName
  if ($name) { $exe = Join-Path $installDir $name }
}
Check (Test-Path $exe) "CargoFrame is installed ($exe)"
Check (Test-Path $uninstKey) 'listed in Apps & features'
$run = (Get-ItemProperty -Path $runKey -ErrorAction SilentlyContinue).CargoFrame
Check ($run -eq "`"$exe`" --autostart") "starts when you sign in by default: $run"

# 4. CargoFrame opened by itself, with a window (its Settings, on the Setup page the first time)
$up = Wait-For { @(Installed-Procs).Count -ge 1 } 30
Check $up 'CargoFrame was started by the installer'
$window = Wait-For { $p = Installed-Procs | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | Select-Object -First 1; [bool]$p } 45
Check $window 'CargoFrame opened a window'
Start-Sleep -Seconds 4
Shot 'installer-4-app-open'
$title = (Installed-Procs | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | Select-Object -First 1).MainWindowTitle
Write-Host "app window: '$title'"

# 5. Clean up: ask it to quit like the installer does, then remove it
if (Test-Path $exe) { Start-Process -FilePath $exe -ArgumentList '--quit' -Wait -ErrorAction SilentlyContinue }
Start-Sleep -Seconds 2
Installed-Procs | Stop-Process -Force -ErrorAction SilentlyContinue
if (Test-Path (Join-Path $installDir 'uninstall.exe')) {
  Start-Process -FilePath (Join-Path $installDir 'uninstall.exe') -ArgumentList @('/S', "_?=$installDir") -Wait | Out-Null
}
Remove-Item $settingsDir -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item $installDir -Recurse -Force -ErrorAction SilentlyContinue

if ($failures.Count -gt 0) {
  Write-Host "`n$($failures.Count) check(s) failed:"
  $failures | ForEach-Object { Write-Host " - $_" }
  exit 1
}
Write-Host "`nThe installer works with one click."
