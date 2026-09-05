# High battery alerts

The daemon notifies when the battery is charging and at or above `--above` (default 85), so the
user knows to unplug. Discharging at 95% is not a high battery (`src/main.rs:166`). `--no-above`
turns the path off.

## Sub-features

- Notify at or above the threshold while charging.
- Stay silent below the threshold, and silent at any charge while discharging.
- The boundary is inclusive: exactly `--above` notifies.
- `--no-above` suppresses high alerts without affecting low alerts.

## How to get to it (user POV)

```sh
rustcharge --sound-path ~/alert.wav --above 85
```

Plug in and let the battery fill. At 85% a notification appears reading "Battery Status: Charging"
with the charge, repeating until the attempts run out.

## Driving it with drive.sh

```sh
# Fires: 92% and charging is above 85.
drive.sh --name high-fires --status Charging --percent 92 --seconds 6 \
  -- --above 85 --no-below --sec 1 --notify-attempts 2

# Boundary: exactly 85 must still fire.
drive.sh --name high-boundary --status Charging --percent 85 --seconds 6 \
  -- --above 85 --no-below --sec 1 --notify-attempts 2

# Silent: high charge but discharging.
drive.sh --name high-discharging-silent --status Discharging --percent 92 --seconds 6 \
  -- --above 85 --no-below --sec 1 --notify-attempts 2
```

## Gotchas

- Only `State::Charging` counts. `Full` and `Unknown` are not charging, so a battery reporting
  `Full` at 100% notifies nothing. That is current behavior; drive `--status Full --percent 100` if
  a change claims to address it.
- Both thresholds are live by default, so a run that omits `--no-below` can produce low alerts too
  and muddy the evidence. Disable the path you are not testing.
