#!/usr/bin/env bash
# Spike S9 on Linux: what does a screen recording cost? Runs ffmpeg on the screen for a few
# seconds with each encoder it can find and reports wall time, CPU time, peak memory and size.
#
#   scripts/e2e/s9-video-linux.sh [seconds] [fps]       (default: 10 s at 30 fps)
#
# * With no DISPLAY it records a virtual 1920x1080 screen (xvfb-run). That proves the script and
#   gives a floor; the numbers that matter come from your own desktop, so run it there:
#   an X11 session records the whole screen with x11grab.
# * On Wayland, x11grab only sees X11 (XWayland) windows, so it measures nothing useful. See
#   docs/spikes.md S9 for how to measure there.
# * CPU is "user + system" seconds of ffmpeg divided by wall time and the number of cores.
#
# Needs: ffmpeg, python3 (to read CPU time and peak memory of the child), xvfb-run (optional).
set -euo pipefail

if [ -z "${CARGOFRAME_S9_INNER:-}" ] && [ -z "${DISPLAY:-}" ]; then
  export CARGOFRAME_S9_INNER=1
  exec xvfb-run -a -s "-screen 0 1920x1080x24" "$0" "$@"
fi

seconds="${1:-10}"
fps="${2:-30}"
command -v ffmpeg >/dev/null || { echo "ffmpeg is not installed" >&2; exit 1; }
command -v python3 >/dev/null || { echo "python3 is not installed" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
log="${CARGOFRAME_S9_LOG:-s9-video-linux.log}"
cores="$(nproc)"
size="$(xdpyinfo 2>/dev/null | awk '/dimensions:/ {print $2; exit}')"
size="${size:-1920x1080}"
encoders="$(ffmpeg -hide_banner -encoders 2>/dev/null)"

# name | extra input options | video filter | encoder options
variants=()
variants+=("x11grab + libx264 ultrafast||format=yuv420p|-c:v libx264 -preset ultrafast -crf 28")
variants+=("x11grab + libx264 veryfast||format=yuv420p|-c:v libx264 -preset veryfast -crf 23")
if grep -q h264_vaapi <<<"$encoders" && [ -e /dev/dri/renderD128 ]; then
  variants+=("x11grab + h264_vaapi|-vaapi_device /dev/dri/renderD128|format=nv12,hwupload|-c:v h264_vaapi -qp 24")
fi
if grep -q h264_nvenc <<<"$encoders" && command -v nvidia-smi >/dev/null; then
  variants+=("x11grab + h264_nvenc||format=yuv420p|-c:v h264_nvenc -preset p1 -cq 28")
fi

measure() { # prints: wall cpu_seconds peak_mb exit
  python3 - "$@" <<'PY'
import resource, subprocess, sys, time
t = time.time()
rc = subprocess.run(sys.argv[1:], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL).returncode
wall = time.time() - t
r = resource.getrusage(resource.RUSAGE_CHILDREN)
print(f"{wall:.2f} {r.ru_utime + r.ru_stime:.2f} {r.ru_maxrss / 1024:.0f} {rc}")
PY
}

{
  echo "# S9 on Linux, $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "ffmpeg: $(ffmpeg -version | head -1)"
  echo "cpu: $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2- | xargs) ($cores cores)"
  echo "screen: $size, ${seconds} s at ${fps} fps, session: ${XDG_SESSION_TYPE:-unknown}"
  echo
  echo "| variant | wall s | CPU s | CPU % of machine | peak MB | clip MB | exit |"
  echo "|---|---|---|---|---|---|---|"
} | tee "$log"

for v in "${variants[@]}"; do
  IFS='|' read -r name pre vf enc <<<"$v"
  out="$work/clip.mp4"
  # shellcheck disable=SC2086
  read -r wall cpu mb rc < <(measure ffmpeg -hide_banner -loglevel error -y $pre \
      -f x11grab -framerate "$fps" -video_size "$size" -i "${DISPLAY}" -t "$seconds" \
      -vf "$vf" $enc "$out")
  mb_out="n/a"
  [ -f "$out" ] && mb_out="$(python3 -c "import os,sys; print(f'{os.path.getsize(sys.argv[1])/1e6:.1f}')" "$out")"
  pct="$(python3 -c "print(f'{$cpu / ($wall * $cores) * 100:.1f}')")"
  echo "| $name | $wall | $cpu | $pct | $mb | $mb_out | $rc |" | tee -a "$log"
  rm -f "$out"
done
echo | tee -a "$log"
echo "Written to $log. Add the CPU model, the GPU and the display setup to the S9 table by hand." | tee -a "$log"
