#!/usr/bin/env bash
# Idle cost of the always-on process: memory and CPU while CargoFrame sits in the tray with no
# window open. Budgets come from the performance plan (under 40 MB, effectively 0% CPU); the gate
# allows 20% over, times CARGOFRAME_PERF_SCALE (default 1) for slower machines.
#
# What "memory" means here: the gate is on ANONYMOUS memory (heap and stacks: what CargoFrame
# itself allocates). Resident size (RSS) is reported too, but on Linux it also counts the pages of
# GTK, WebKit and Mesa libraries that every GTK program maps when it opens an X display, which
# CargoFrame cannot shrink and which other programs share. docs/performance.md has the numbers.
#
#   scripts/e2e/linux-idle.sh [path-to-release-binary]
#
# Needs: xvfb, xdotool, dbus-run-session, and the stub built with
#   cargo build -p cargoframe-capture --example tray_host_stub
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

if [ -z "${CARGOFRAME_IDLE_INNER:-}" ]; then
  bin="${1:-$root/target/release/cargoframe}"
  export CARGOFRAME_IDLE_INNER=1 CARGOFRAME_IDLE_BIN="$(readlink -f "$bin")"
  exec dbus-run-session -- xvfb-run -a -s "-screen 0 1280x800x24" "$0"
fi

bin="$CARGOFRAME_IDLE_BIN"
stub="$root/target/debug/examples/tray_host_stub"
[ -x "$stub" ] || { echo "build the stub first: cargo build -p cargoframe-capture --example tray_host_stub" >&2; exit 1; }
scale="${CARGOFRAME_PERF_SCALE:-1}"

work="$(mktemp -d)"
cleanup() {
  [ -n "${app_pid:-}" ] && kill "$app_pid" 2>/dev/null || true
  [ -n "${stub_pid:-}" ] && kill "$stub_pid" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT
fail() { echo "FAIL: $*" >&2; [ -f "$work/app.log" ] && tail -n 30 "$work/app.log" >&2; exit 1; }

export HOME="$work/home"; mkdir -p "$HOME"
export CARGOFRAME_HOME="$work/cfg" CARGOFRAME_LOG=info CARGOFRAME_LOG_STDERR=1
export WEBKIT_DISABLE_COMPOSITING_MODE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 GDK_BACKEND=x11

"$stub" > "$work/stub.out" 2>&1 &
stub_pid=$!
for _ in $(seq 50); do grep -q ready "$work/stub.out" 2>/dev/null && break; sleep 0.1; done
grep -q ready "$work/stub.out" || fail "the tray host stub did not start"

"$bin" --autostart > "$work/app.log" 2>&1 &
app_pid=$!
sleep 4
kill -0 "$app_pid" 2>/dev/null || fail "CargoFrame exited at start-up"
# Window titles match case-insensitively and by substring in xdotool (the tray library keeps an
# invisible helper window named "cargoframe"), so compare the exact titles ourselves.
for w in $(xdotool search --name 'cargoframe' 2>/dev/null || true); do
  title="$(xdotool getwindowname "$w" 2>/dev/null || true)"
  case "$title" in
    "CargoFrame"|"CargoFrame settings") fail "the window \"$title\" is open: with a tray present CargoFrame should start tray-only" ;;
  esac
done

rss_kb() { awk '/^VmRSS:/ {print $2}' "/proc/$app_pid/status"; }
smaps_kb() { awk -v k="$1:" '$1 == k {print $2}' "/proc/$app_pid/smaps_rollup"; }
cpu_ticks() { awk '{print $14 + $15}' "/proc/$app_pid/stat"; }   # user + system, in clock ticks
hz="$(getconf CLK_TCK)"

rss0="$(rss_kb)"
t0="$(cpu_ticks)"
window=15
sleep "$window"
t1="$(cpu_ticks)"
rss1="$(rss_kb)"
pss1="$(smaps_kb Pss)"
anon1="$(smaps_kb Anonymous)"
kill -0 "$app_pid" 2>/dev/null || fail "CargoFrame exited while idle"

mb() { awk -v k="$1" 'BEGIN { printf "%.1f", k / 1024 }'; }
rss_mb="$(mb "$rss1")"; pss_mb="$(mb "$pss1")"; anon_mb="$(mb "$anon1")"
cpu_pct="$(awk -v a="$t0" -v b="$t1" -v hz="$hz" -v w="$window" 'BEGIN { printf "%.2f", (b - a) / hz / w * 100 }')"
threads="$(awk '/^Threads:/ {print $2}' "/proc/$app_pid/status")"
echo "tray-only idle: ${anon_mb} MB anonymous (gated), ${pss_mb} MB proportional, ${rss_mb} MB resident (${rss0} kB at start), ${cpu_pct}% CPU over ${window}s, ${threads} threads"

mem_gate="$(awk -v s="$scale" 'BEGIN { printf "%.1f", 40 * 1.2 * s }')"
cpu_gate="$(awk -v s="$scale" 'BEGIN { printf "%.2f", 0.5 * s }')"
awk -v v="$anon_mb" -v g="$mem_gate" 'BEGIN { exit !(v <= g) }' || fail "anonymous memory ${anon_mb} MB is over the gate of ${mem_gate} MB (40 MB budget + 20%, x${scale})"
awk -v v="$cpu_pct" -v g="$cpu_gate" 'BEGIN { exit !(v <= g) }' || fail "idle CPU ${cpu_pct}% is over the gate of ${cpu_gate}%"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  {
    echo "### Tray-only idle (Linux)"
    echo "- Anonymous memory: ${anon_mb} MB (gate ${mem_gate} MB)"
    echo "- Proportional (PSS): ${pss_mb} MB, resident (RSS): ${rss_mb} MB (shared library pages; not gated)"
    echo "- CPU: ${cpu_pct}% over ${window}s (gate ${cpu_gate}%)"
  } >> "$GITHUB_STEP_SUMMARY"
fi

"$bin" --quit >/dev/null 2>&1 || true
for _ in $(seq 50); do kill -0 "$app_pid" 2>/dev/null || break; sleep 0.1; done
app_pid=""
echo "Linux idle check passed"
