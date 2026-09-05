# Low battery alerts

The daemon notifies when the battery is discharging and at or below `--below` (default 20). Both
conditions must hold: a battery sitting at 5% while plugged in and charging is not a low battery
(`src/main.rs:168`). `--no-below` turns the whole path off.

## Sub-features

- Notify at or below the threshold while discharging.
- Stay silent above the threshold, and silent at any charge while charging.
- The boundary is inclusive: exactly `--below` notifies.
- `--no-below` suppresses low alerts without affecting high alerts.

## How to get to it (user POV)

Run the daemon and let the battery drain past the threshold:

```sh
rustcharge --sound-path ~/alert.wav --below 20
```

A desktop notification appears reading "Battery Status: Discharging" with the current charge, and
the sound plays. It repeats every `--sec` until the attempt budget runs out.

## Driving it with drive.sh

```sh
# Fires: 12% and discharging is below 20.
drive.sh --name low-fires --status Discharging --percent 12 --seconds 9 \
  -- --below 20 --no-above --sec 1 --notify-attempts 10

# Boundary: exactly 20 must still fire.
drive.sh --name low-boundary --status Discharging --percent 20 --seconds 6 \
  -- --below 20 --no-above --sec 1 --notify-attempts 2

# Silent: same low charge, but charging.
drive.sh --name low-charging-silent --status Charging --percent 12 --seconds 6 \
  -- --below 20 --no-above --sec 1 --notify-attempts 2

# Silent: --no-below disables the path entirely.
drive.sh --name low-disabled --status Discharging --percent 5 --seconds 6 \
  -- --below 20 --no-below --no-above --sec 1 --notify-attempts 2
```

## Gotchas

- The silent cases prove a negative, so they are only meaningful if a positive case ran in the
  same session with the same harness. A broken D-Bus connection also produces zero notifications.
  Check `stderr.log` is empty and pair every silent run with a firing one.
- `21` does not fire and `20` does. Off-by-one changes here are invisible unless the boundary run
  is included.
- A run shorter than about three seconds can end before the first notification. That is a harness
  artifact, not a pass.
