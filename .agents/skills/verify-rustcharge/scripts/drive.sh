#!/usr/bin/env bash
# Drive rustcharge against a fake battery and capture proof.
#
# Usage:
#   drive.sh --name <label> --status <Charging|Discharging> --percent <0-100> \
#            --seconds <run length> [--mutate SEC:STATUS:PERCENT]... \
#            [--real-battery] -- <rustcharge args...>
#
# Example:
#   drive.sh --name low-battery --status Discharging --percent 12 --seconds 8 \
#            --mutate 5:Discharging:60 \
#            -- --below 20 --no-above --sec 1 --notify-attempts 3
#
# Evidence lands in verification-evidence/<utc>-<name>/ and survives cleanup.
# Env: RC_SOUND overrides the sound file (default: a synthesized 22050 Hz beep,
# whose odd sample rate makes rustcharge's audio stream identifiable in pactl).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../../../.." && pwd)"
bin="$repo/target/release/rustcharge"

name=""; status="Discharging"; percent=20; seconds=8; real=0; mutations=()
while [ $# -gt 0 ]; do
  case "$1" in
    --name)     name="$2"; shift 2 ;;
    --status)   status="$2"; shift 2 ;;
    --percent)  percent="$2"; shift 2 ;;
    --seconds)  seconds="$2"; shift 2 ;;
    --mutate)   mutations+=("$2"); shift 2 ;;
    --real-battery) real=1; shift ;;
    --) shift; break ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done
[ -n "$name" ] || { echo "--name is required" >&2; exit 2; }
[ -x "$bin" ] || { echo "binary missing, run: cargo build --release" >&2; exit 2; }
[ $# -gt 0 ] || { echo "pass rustcharge args after --" >&2; exit 2; }

out="$repo/verification-evidence/$(date -u +%Y%m%dT%H%M%SZ)-$name"
mkdir -p "$out"
scratch="$(mktemp -d /tmp/rustcharge-verify.XXXXXX)"

sound="${RC_SOUND:-}"
if [ -z "$sound" ]; then
  sound="$scratch/beep.wav"
  ffmpeg -y -f lavfi -i "sine=frequency=880:duration=0.3" -ac 1 -ar 22050 "$sound" >/dev/null 2>&1
fi

pids=()
cleanup() {
  # Kill only what this run started, by recorded PID. Never by process name:
  # the user's own rustcharge daemon is very likely running.
  [ -n "${pgid:-}" ] && kill -TERM -- "-$pgid" 2>/dev/null || true
  for p in "${pids[@]:-}"; do [ -n "$p" ] && kill "$p" 2>/dev/null || true; done
  sleep 0.3
  [ -n "${pgid:-}" ] && kill -9 -- "-$pgid" 2>/dev/null || true
  for p in "${pids[@]:-}"; do [ -n "$p" ] && kill -9 "$p" 2>/dev/null || true; done
  rm -rf "$scratch"
}
trap cleanup EXIT

{
  echo "binary:   $bin"
  echo "args:     $*"
  echo "sound:    $sound"
  echo "battery:  $status ${percent}% (fake=$([ $real -eq 1 ] && echo no || echo yes))"
  echo "seconds:  $seconds"
  echo "mutations: ${mutations[*]:-none}"
} > "$out/command.txt"

dbus-monitor --session "interface='org.freedesktop.Notifications',member='Notify'" \
  > "$out/notifications.raw" 2>&1 &
pids+=($!)

( while :; do
    pactl list sink-inputs 2>/dev/null \
      | grep -E 'Sink Input #|application\.name =|Sample Specification' || true
    sleep 0.2
  done ) > "$out/audio-sinks.raw" 2>&1 &
pids+=($!)

sleep 1

# setsid puts the app in its own process group, so cleanup and the audio
# matcher can both address exactly our processes -- never the user's own daemon.
# Tag this run's audio stream. rodio reaches the server through the ALSA
# plugin, which publishes no PID, so the tag is the only way to tell our
# playback apart from the user's own rustcharge daemon.
tag="rustcharge-verify-$$"
export PIPEWIRE_PROPS="{ application.name = \"$tag\" }"
export PULSE_PROP="application.name=$tag"
echo "$tag" > "$out/audio-tag.txt"

if [ $real -eq 1 ]; then
  setsid "$bin" --sound-path "$sound" "$@" > "$out/stdout.log" 2> "$out/stderr.log" &
  app=$!
  echo "real battery: $(cat /sys/class/power_supply/BAT*/status) $(cat /sys/class/power_supply/BAT*/capacity)%" \
    > "$out/battery-timeline.log"
else
  "$here/fake-battery.sh" "$scratch/sysfs" "$status" "$percent" > "$out/battery-timeline.log"
  # --die-with-parent matters: killing bwrap alone leaves rustcharge orphaned.
  setsid bwrap --die-with-parent --dev-bind / / --bind "$scratch/sysfs" /sys/class/power_supply \
    -- "$bin" --sound-path "$sound" "$@" > "$out/stdout.log" 2> "$out/stderr.log" &
  app=$!
fi
pgid=$(ps -o pgid= -p "$app" 2>/dev/null | tr -d ' ')

start=$(date +%s)
for m in "${mutations[@]:-}"; do
  [ -n "$m" ] || continue
  IFS=: read -r at st pct <<< "$m"
  now=$(date +%s)
  wait_for=$(( start + at - now ))
  [ "$wait_for" -gt 0 ] && sleep "$wait_for"
  if [ $real -eq 1 ]; then
    echo "t+${at}s SKIPPED mutation $st $pct% (real battery is not writable)" >> "$out/battery-timeline.log"
  else
    echo "t+${at}s mutate -> $st ${pct}%" >> "$out/battery-timeline.log"
    "$here/fake-battery.sh" "$scratch/sysfs" "$st" "$pct" >> "$out/battery-timeline.log"
  fi
done

now=$(date +%s)
remaining=$(( start + seconds - now ))
[ "$remaining" -gt 0 ] && sleep "$remaining"

cleanup
trap - EXIT
sleep 0.5

# Parse the raw bus trace into one line per notification rustcharge sent.
# Only method calls addressed to the daemon are ours; the daemon re-emits copies.
awk '
  /member=Notify/ { want = (/destination=org.freedesktop.Notifications/) ? 1 : 0; n = 0; urg = "-"; next }
  want && /^[[:space:]]*string "/ { n++; line = $0; sub(/^[[:space:]]*string "/, "", line); sub(/"$/, "", line); f[n] = line; next }
  want && /variant[[:space:]]+byte/ { urg = $NF; next }
  want && /^[[:space:]]*int32/ { printf "app=%s | summary=%s | body=%s | urgency=%s\n", f[1], f[3], f[4], urg; want = 0 }
' "$out/notifications.raw" > "$out/notifications.txt"

# A sink-input counts as ours only if it carries this run's tag. Matching the
# name "rustcharge" alone would also catch the user's own daemon.
awk -v tag="$tag" '
  /Sink Input #/          { id = $3; spec = "" }
  /Sample Specification:/ { spec = $0; sub(/^[[:space:]]+/, "", spec) }
  index($0, tag)          { print id, spec }
' "$out/audio-sinks.raw" | sort -u > "$out/audio-streams.txt" || true

if [ ! -s "$out/audio-streams.txt" ] && grep -q 'rustcharge' "$out/audio-sinks.raw" 2>/dev/null; then
  echo "UNTAGGED rustcharge audio streams seen -- the audio server ignored PIPEWIRE_PROPS/PULSE_PROP," \
       "so these cannot be told apart from the user's own daemon:" > "$out/audio-streams.txt"
  grep -c 'rustcharge' "$out/audio-sinks.raw" >> "$out/audio-streams.txt"
fi

count=$(wc -l < "$out/notifications.txt")
{
  echo "notifications sent: $count"
  echo "distinct payloads:"
  sort -u "$out/notifications.txt" | sed 's/^/  /'
  echo "stderr: $([ -s "$out/stderr.log" ] && echo "NON-EMPTY (see stderr.log)" || echo empty)"
  echo "audio streams matched: $(wc -l < "$out/audio-streams.txt")"
} > "$out/summary.txt"

cat "$out/summary.txt"
echo
echo "evidence: $out"
