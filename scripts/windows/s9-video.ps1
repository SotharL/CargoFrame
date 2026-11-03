<#
  Spike S9 on Windows: what does a screen recording cost on this PC?

  Records the primary screen for a few seconds with every capture and encoder combination this
  ffmpeg can run, and reports wall time, CPU time, peak memory and the size of the clip. The
  result goes to s9-video.log in the repository folder and is also printed.

    powershell -ExecutionPolicy Bypass -File scripts\windows\s9-video.ps1
    powershell -ExecutionPolicy Bypass -File scripts\windows\s9-video.ps1 -Seconds 10 -Fps 30 -Ffmpeg C:\tools\ffmpeg\bin\ffmpeg.exe

  Needs ffmpeg (for example: winget install Gyan.FFmpeg). Move things on the screen while it
  runs: a still screen costs far less than a busy one. Open Task Manager, Performance, GPU, and
  watch "Video Encode" and "3D" during each run; GPU load is not measured here, so write what you
  see into the S9 table by hand.

  This script has not been run by its author (no Windows machine in the build environment): if it
  fails, the first lines of the log say how.
#>
param(
  [int]$Seconds = 10,
  [int]$Fps = 30,
  [string]$Ffmpeg = "ffmpeg"
)

$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$log = Join-Path $root "s9-video.log"
$work = Join-Path $env:TEMP "cargoframe-s9"
New-Item -ItemType Directory -Force -Path $work | Out-Null

try { $version = (& $Ffmpeg -hide_banner -version 2>&1 | Select-Object -First 1) }
catch { throw "ffmpeg was not found. Install it (winget install Gyan.FFmpeg) or pass -Ffmpeg <path>." }
$encoders = (& $Ffmpeg -hide_banner -encoders 2>&1 | Out-String)
$cores = [Environment]::ProcessorCount
$cpu = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name
$gpu = (Get-CimInstance Win32_VideoController | ForEach-Object { $_.Name }) -join ", "
Add-Type -AssemblyName System.Windows.Forms
$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$size = "$($bounds.Width)x$($bounds.Height)"

# Two ways to get the picture: gdigrab (the old, CPU-based way) and ddagrab (Desktop Duplication,
# the picture stays on the GPU until it is downloaded for the encoder).
$grabs = @(
  @{ Name = "gdigrab"; Input = @("-f", "gdigrab", "-framerate", "$Fps", "-i", "desktop"); Filter = "format=yuv420p" },
  @{ Name = "ddagrab"; Input = @("-f", "lavfi", "-i", "ddagrab=output_idx=0:framerate=$Fps"); Filter = "hwdownload,format=bgra,format=yuv420p" }
)
$codecs = @(
  @{ Name = "libx264 ultrafast"; Args = @("-c:v", "libx264", "-preset", "ultrafast", "-crf", "28"); Have = $true },
  @{ Name = "libx264 veryfast"; Args = @("-c:v", "libx264", "-preset", "veryfast", "-crf", "23"); Have = $true },
  @{ Name = "h264_nvenc"; Args = @("-c:v", "h264_nvenc", "-preset", "p1", "-cq", "28"); Have = ($encoders -match "h264_nvenc") },
  @{ Name = "h264_qsv"; Args = @("-c:v", "h264_qsv", "-global_quality", "28"); Have = ($encoders -match "h264_qsv") },
  @{ Name = "h264_amf"; Args = @("-c:v", "h264_amf", "-quality", "speed"); Have = ($encoders -match "h264_amf") },
  @{ Name = "h264_mf (Windows built-in)"; Args = @("-c:v", "h264_mf"); Have = ($encoders -match "h264_mf") }
)

$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("# S9 on Windows, $(Get-Date -Format s)")
$lines.Add("ffmpeg: $version")
$lines.Add("cpu: $cpu ($cores logical cores)")
$lines.Add("gpu: $gpu")
$lines.Add("screen: $size, $Seconds s at $Fps fps")
$lines.Add("")
$lines.Add("| capture + encoder | wall s | CPU s | CPU % of machine | peak MB | clip MB | dup/drop | result |")
$lines.Add("|---|---|---|---|---|---|---|---|")

foreach ($g in $grabs) {
  foreach ($c in $codecs) {
    if (-not $c.Have) { continue }
    $name = "$($g.Name) + $($c.Name)"
    $out = Join-Path $work "clip.mp4"
    $err = Join-Path $work "ffmpeg.err"
    Remove-Item $out, $err -ErrorAction SilentlyContinue
    $ffArgs = @("-hide_banner", "-y") + $g.Input + @("-t", "$Seconds", "-vf", $g.Filter) + $c.Args + @("`"$out`"")
    $started = Get-Date
    $p = Start-Process -FilePath $Ffmpeg -ArgumentList $ffArgs -NoNewWindow -PassThru -RedirectStandardError $err
    $peak = 0
    while (-not $p.HasExited) {
      try { $p.Refresh(); if ($p.PeakWorkingSet64 -gt $peak) { $peak = $p.PeakWorkingSet64 } } catch { }
      Start-Sleep -Milliseconds 250
    }
    $p.WaitForExit()
    $wall = ((Get-Date) - $started).TotalSeconds
    $cpuSeconds = 0
    try { $cpuSeconds = $p.TotalProcessorTime.TotalSeconds } catch { }
    $dup = "?"
    if (Test-Path $err) {
      $stats = Select-String -Path $err -Pattern "dup=(\d+) drop=(\d+)" | Select-Object -Last 1
      if ($stats) { $dup = "$($stats.Matches[0].Groups[1].Value)/$($stats.Matches[0].Groups[2].Value)" }
    }
    $clipMb = if (Test-Path $out) { "{0:N1}" -f ((Get-Item $out).Length / 1MB) } else { "n/a" }
    $result = if ($p.ExitCode -eq 0) { "ok" } else {
      $why = (Get-Content $err -ErrorAction SilentlyContinue | Where-Object { $_.Trim() } | Select-Object -Last 1)
      "exit $($p.ExitCode): $why"
    }
    $pct = if ($wall -gt 0) { "{0:N1}" -f ($cpuSeconds / ($wall * $cores) * 100) } else { "?" }
    $wallText = "{0:N1}" -f $wall
    $cpuText = "{0:N1}" -f $cpuSeconds
    $peakText = "{0:N0}" -f ($peak / 1MB)
    $lines.Add("| $name | $wallText | $cpuText | $pct | $peakText | $clipMb | $dup | $result |")
    Write-Host $lines[$lines.Count - 1]
  }
}

$lines.Add("")
$lines.Add("Add the GPU load you saw in Task Manager for each run, and the display setup, to the S9 table in docs\spikes.md.")
$lines | Set-Content -Encoding UTF8 $log
Write-Host ""
Write-Host "Written to $log"
Remove-Item $work -Recurse -ErrorAction SilentlyContinue
