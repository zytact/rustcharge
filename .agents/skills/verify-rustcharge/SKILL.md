---
name: verify-rustcharge
description: Drive the real rustcharge CLI daemon against a fake battery and capture proof that it notified, at the right threshold, with the right urgency and sound. Use when verifying a change to the polling loop, threshold logic, notification session state machine, urgency mapping, sound playback, or CLI arguments.
---

# Verify rustcharge

Rustcharge is a long-running CLI daemon. It polls the battery, and when the charge crosses a
threshold it sends a desktop notification over D-Bus and plays a sound. There is no UI to click.
Verification means: put the battery in a chosen state, run the real binary, and read what it
actually put on the session bus and the audio server.

The battery is real hardware and the `battery` crate hardcodes `/sys/class/power_supply`
(`battery-0.7.8/src/platform/linux/manager.rs:9`). So the harness bind-mounts a fake sysfs tree
over that path inside a `bwrap` namespace. Nothing outside the namespace sees it, the machine's
real battery is untouched, and every threshold, charging state, and mid-run transition becomes
drivable on demand.

## Do not kill rustcharge by name

The user runs rustcharge as their own daily battery monitor. `pkill rustcharge` kills it.
Every script here tracks its own process group and kills only that. Keep it that way.

## Launch

There is no server to keep alive. Each drive starts a fresh short-lived instance in its own
process group and namespace, so drives can run back to back and never share state.

Build once:

```sh
cargo build --release
```

Ready when `target/release/rustcharge` exists and is newer than everything in `src/`. `doctor.sh`
checks exactly that.

Teardown is automatic: `drive.sh` traps EXIT and kills its own process group. `bwrap` runs with
`--die-with-parent`, which matters because killing `bwrap` alone leaves the child rustcharge
orphaned and still notifying.

## Doctor

Run this first whenever anything looks off. It is read-only.

```sh
.agents/skills/verify-rustcharge/scripts/doctor.sh
```

It reports the binary's freshness, `bwrap`, `dbus-monitor`, the session bus, whether a daemon owns
`org.freedesktop.Notifications`, the audio server, and `ffmpeg`. It also lists any foreign
rustcharge processes so you know not to touch them.

The notification daemon check is not optional. `show_notification` calls `.expect()`
(`src/main.rs:133`), so with no daemon the app panics rather than degrading.

## Drive

```sh
.agents/skills/verify-rustcharge/scripts/drive.sh \
  --name <label> \
  --status <Charging|Discharging|Full|Unknown> \
  --percent <0-100> \
  --seconds <run length> \
  [--mutate SEC:STATUS:PERCENT]... \
  [--real-battery] \
  -- <rustcharge args...>
```

`--mutate` rewrites the fake sysfs mid-run, which is how you drive the session state machine: the
app reads the tree fresh on every poll, so a mutation at t+8s is a battery transition at t+8s.

```sh
# Low battery alerts, then the battery recovers into the safe zone at t+5s.
.agents/skills/verify-rustcharge/scripts/drive.sh --name low-battery \
  --status Discharging --percent 12 --seconds 9 --mutate 5:Discharging:60 \
  -- --below 20 --no-above --sec 1 --notify-attempts 10 --urgency 2
```

Use `--sec 1` and small `--notify-attempts`. One notification cycle costs about two seconds: the
polling sleep plus the fixed one-second sleep in `play_sound` (`src/utils/sound.rs:23`). Budget
`--seconds` accordingly or the run ends mid-session and the evidence understates what happened.

`--real-battery` skips the namespace and drives the machine's actual battery. Only reach for it to
confirm the fake tree is not lying, and pick thresholds around the current real charge
(`cat /sys/class/power_supply/BAT*/capacity`). `--mutate` is a no-op there and says so in the
timeline.

CLI argument behavior needs none of this. clap rejects bad input before the loop starts, so run
the binary directly and redirect into the evidence directory.

## Evidence

Every run writes `verification-evidence/<utc-timestamp>-<name>/`:

| File | What it proves |
| --- | --- |
| `summary.txt` | notification count, distinct payloads, stderr state, audio stream count |
| `notifications.txt` | one parsed line per notification: app, summary, body, urgency |
| `notifications.raw` | the unedited `dbus-monitor` trace behind it |
| `audio-streams.txt` | sink-inputs this run created, matched by a per-run tag |
| `battery-timeline.log` | the fake battery's state and every mutation, with timestamps |
| `stderr.log` | playback and battery-read failures; empty is the passing state |
| `command.txt` | the exact binary, args, sound file, and battery state used |

Proof standards for this repo:

- Drive the real binary through its real polling loop. Do not call the state machine directly or
  add a test-only hook to trigger a notification.
- Capture the transition, not just the end state. `--mutate` plus `battery-timeline.log` is what
  makes "it stopped notifying" distinguishable from "it never started".
- Verify the side effects too. A notification that reaches D-Bus but plays no sound is a
  regression, and only `audio-streams.txt` catches it. `notifications.txt` alone is incomplete.
- Read `notifications.txt`, not the count. The urgency byte and the exact body string are where
  regressions hide, and `Charge: {:.0}%` rounds, so 12.4% and 12% look identical.
- The fake battery is not a mock of the app. It is a mock of the kernel, one layer below the crate
  boundary the app already depends on. Nothing in rustcharge is stubbed.

`dbus-monitor` sees each notification twice: once as rustcharge's method call to
`org.freedesktop.Notifications`, and again as the daemon forwarding it to the shell. The parser
keeps only the first. If you read `notifications.raw` by hand, expect the duplicates.

rodio reaches the audio server through the ALSA plugin, which publishes no PID. The harness tags
the stream with `PIPEWIRE_PROPS`/`PULSE_PROP` (`audio-tag.txt`) so it can be told apart from the
user's own daemon. If the audio server ignores those, `audio-streams.txt` says so explicitly
rather than counting streams that might not be ours.

## Cleanup

`drive.sh` cleans up after itself, including on failure, and removes its scratch sysfs tree from
`/tmp`. After any run that was interrupted, confirm nothing was stranded:

```sh
pgrep -af rustcharge | awk '$2 ~ /\/rustcharge$/'
```

Anything under `target/` is yours and should be gone. Anything else is the user's daemon. Leave it.

Evidence under `verification-evidence/` is never touched by cleanup. Delete old runs by hand when
you no longer need them.

## Helpers

| Script | Invocation |
| --- | --- |
| `scripts/doctor.sh` | `.agents/skills/verify-rustcharge/scripts/doctor.sh` |
| `scripts/drive.sh` | see Drive above |
| `scripts/fake-battery.sh` | `.agents/skills/verify-rustcharge/scripts/fake-battery.sh <sysfs-dir> <status> <percent>` |

`fake-battery.sh` is called by `drive.sh`; call it directly only when hand-building a scenario
`drive.sh` cannot express.

## Feature map

`features/` lists what a user can actually do and how to prove each one. Read
[`features/README.md`](features/README.md) first. A proof that drives only the low-battery path is
incomplete when the map lists five features.
