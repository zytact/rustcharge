# Notification content and urgency

Every notification carries the app name, a summary naming the charging state, a body with the
rounded charge, and on Linux an urgency byte from `--urgency` (`src/main.rs:116-147`).

## Sub-features

- App name is always `Rustcharge`.
- Summary is `Battery Status: Charging` or `Battery Status: Discharging`.
- Body is `Charge: N%`, rounded to a whole number.
- `--urgency` maps 0 to Low, 1 to Normal (default), 2 to Critical, and rides on the notification
  as the `urgency` hint.
- Urgency is Linux-only. The flag does not exist on other platforms and the hint is not sent.

## How to get to it (user POV)

Any alert. A critical notification stays on screen in GNOME until dismissed; a low one fades. That
difference is the reason the flag exists.

## Driving it with drive.sh

```sh
for u in 0 1 2; do
  drive.sh --name "urgency-$u" --status Discharging --percent 12 --seconds 6 \
    -- --below 20 --no-above --sec 1 --notify-attempts 2 --urgency "$u"
done
```

Check `notifications.txt` in each run. One line per notification:

```
app=Rustcharge | summary=Battery Status: Discharging | body=Charge: 12% | urgency=2
```

Charging-state wording comes from the same field the threshold logic reads, so drive it both ways:

```sh
drive.sh --name content-charging --status Charging --percent 92 --seconds 6 \
  -- --above 85 --no-below --sec 1 --notify-attempts 2
```

## Gotchas

- `{:.0}` rounds rather than truncates, so 12.5% renders as `Charge: 13%`. The fake battery derives
  the charge from `charge_now / charge_full`, so whole percentages come out exact. Fractional
  percentages are only reachable with `--real-battery`.
- The parser reports `urgency=-` when no hint was sent. On Linux that is a regression, not a
  formatting quirk.
- `notify-rust` opens a new bus connection per notification, so each one appears under a different
  sender in `notifications.raw`. That is normal.
- Any change to urgency must stay behind `#[cfg(target_os = "linux")]` on both the CLI field and
  the notification builder. The harness cannot catch a break on macOS or Windows; check the cfg
  gates by reading, and confirm the release workflow still builds all four targets.
