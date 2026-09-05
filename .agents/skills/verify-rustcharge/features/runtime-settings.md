# Runtime settings

An existing monitor accepts authenticated local `set` and `status` commands. Accepted changes are written to the per-user config file. Threshold changes trigger an immediate battery evaluation.

## Sub-features

- `above`, `below`, `above-enabled`, and `below-enabled` change threshold behavior while the monitor runs.
- `sound-path`, `sec`, and `notify-attempts` update their matching monitor settings. Linux also supports `urgency`.
- `status` reports the effective live configuration.
- `status`, sound, urgency, and notification-attempt changes preserve the next scheduled battery check. Changing `sec` starts a new interval.
- Invalid values and persistence failures leave the live setting unchanged.
- Disabling a threshold ends its alert session and cancels queued sound for that threshold. Enabling it allows a fresh session.
- Sound and urgency changes preserve the current notification attempt count.
- Runtime commands require an existing monitor and never start one.

## How to get to it

Start the monitor with a long polling interval, then run commands from another terminal:

```sh
rustcharge --sound-path ~/alert.wav --sec 120
rustcharge set above 90
rustcharge set above-enabled false
rustcharge status
```

## Driving it with drive.sh

`drive.sh` owns the monitor process for a single command and cannot currently issue a second command while that process runs. Drive the monitor with the fake battery as usual, then run `set` and `status` against that process from a separate shell in the same environment. Capture command output, notification evidence, the battery timeline, and the resulting config file together.

Use a long `--sec` value when proving wake behavior. Disable and reenable a threshold while the fake battery remains beyond it to prove the old session ends and a fresh evaluation starts without waiting for a safe-zone transition.

## Gotchas

- Explicit startup flags override persisted settings at startup. A later `set` replaces its named live setting and persists only that setting.
- A sound that is already playing can finish after its threshold is disabled. Queued sound for that threshold is canceled.
- The authenticated endpoint is per user. A stale endpoint is replaced only after its listener cannot be reached.
