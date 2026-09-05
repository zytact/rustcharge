#!/usr/bin/env bash
# Read-only preflight. Answers: can this machine drive rustcharge and capture proof?
# Usage: doctor.sh
set -uo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
bin="$repo/target/release/rustcharge"
fail=0

ok()   { printf 'ok    %s\n' "$1"; }
bad()  { printf 'FAIL  %s\n' "$1"; fail=1; }
warn() { printf 'warn  %s\n' "$1"; }

[ -d "$repo/src" ] && ok "repo root: $repo" || bad "repo root not found (looked at $repo)"

if [ -x "$bin" ]; then
  newest=$(find "$repo/src" "$repo/Cargo.toml" -type f -newer "$bin" 2>/dev/null | head -1)
  if [ -n "$newest" ]; then
    bad "binary is stale, $newest is newer -- run: cargo build --release"
  else
    ok "binary current: $bin (v$(grep -m1 '^version' "$repo/Cargo.toml" | cut -d'"' -f2))"
  fi
else
  bad "binary missing -- run: cargo build --release"
fi

command -v bwrap >/dev/null && ok "bwrap present (fake-battery isolation available)" \
  || bad "bwrap missing -- without it only the real battery state is drivable"

command -v dbus-monitor >/dev/null && ok "dbus-monitor present" || bad "dbus-monitor missing -- no notification evidence"

if [ -n "${DBUS_SESSION_BUS_ADDRESS:-}" ]; then
  ok "session bus: $DBUS_SESSION_BUS_ADDRESS"
else
  bad "DBUS_SESSION_BUS_ADDRESS unset -- notifications will panic"
fi

if gdbus call --session -d org.freedesktop.DBus -o /org/freedesktop/DBus \
     -m org.freedesktop.DBus.NameHasOwner org.freedesktop.Notifications 2>/dev/null | grep -q true; then
  ok "notification daemon owns org.freedesktop.Notifications"
else
  bad "no notification daemon -- show_notification() calls .expect() and will panic"
fi

if command -v pactl >/dev/null && pactl info >/dev/null 2>&1; then
  ok "audio server reachable ($(pactl info | awk -F': ' '/Server Name/ {print $2}'))"
else
  warn "no audio server -- sound evidence unavailable, playback failures print to stderr"
fi

command -v ffmpeg >/dev/null && ok "ffmpeg present (can synthesize the test sound)" \
  || warn "ffmpeg missing -- pass an existing sound file via RC_SOUND"

# The user very likely runs rustcharge as their own daily monitor. Never kill by name.
others=$(pgrep -af 'rustcharge' | awk '$2 ~ /\/rustcharge$/' | grep -v "$repo/target" || true)
if [ -n "$others" ]; then
  warn "other rustcharge processes are running -- these are NOT yours, leave them alone:"
  printf '        %s\n' "$others"
fi

echo
[ "$fail" -eq 0 ] && echo "doctor: ready to drive" || echo "doctor: NOT ready"
exit "$fail"
