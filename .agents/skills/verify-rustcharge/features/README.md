# Rustcharge feature map

What a user can do with rustcharge, and what proves each one works. Keep this in sync with the
code: when a feature is added, changed, or removed, update the file here in the same change.

| Feature | File | Drives |
| --- | --- | --- |
| Low battery alerts | [low-battery-alerts.md](low-battery-alerts.md) | `--below`, `--no-below` |
| High battery alerts | [high-battery-alerts.md](high-battery-alerts.md) | `--above`, `--no-above` |
| Notification session limit | [notification-sessions.md](notification-sessions.md) | `--notify-attempts`, session restart rules |
| Notification content and urgency | [notification-content.md](notification-content.md) | `--urgency`, summary and body text |
| Sound playback | [sound-playback.md](sound-playback.md) | `--sound-path` |
| CLI arguments | [cli-arguments.md](cli-arguments.md) | argument parsing, ranges, defaults, `--help` |
| Runtime settings | [runtime-settings.md](runtime-settings.md) | `set`, `status`, persistence, live session changes |

All of these run through the same harness. Read `../SKILL.md` before driving any of them.

The features are not independent. Every notification carries a threshold decision, a session
decision, a payload, and a sound. A single `drive.sh` run produces evidence for all four, so a
change touching the loop should be verified with runs from several of these files, not one.
