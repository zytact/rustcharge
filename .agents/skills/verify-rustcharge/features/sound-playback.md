# Sound playback

Every notification also plays the file at `--sound-path` through rodio, then sleeps one second to
let it finish (`src/utils/sound.rs`). Failures are printed to stderr and never stop monitoring.

## Sub-features

- A sound plays alongside each notification.
- A missing file prints `Failed to open sound file` and the loop continues.
- An undecodable file prints `Failed to decode audio` and the loop continues.
- The one-second sleep after each play sets the floor on how fast notifications can repeat.

## How to get to it (user POV)

```sh
rustcharge --sound-path ~/alert.wav --below 20
```

The sound plays with each alert. A bad path degrades to silent notifications rather than a crash.

## Driving it with drive.sh

Playback happens by default; `drive.sh` synthesizes a beep unless `RC_SOUND` overrides it. A
passing run has one line in `audio-streams.txt` per line in `notifications.txt`:

```sh
drive.sh --name sound-plays --status Discharging --percent 12 --seconds 9 \
  -- --below 20 --no-above --sec 1 --notify-attempts 3
wc -l verification-evidence/*sound-plays/notifications.txt \
      verification-evidence/*sound-plays/audio-streams.txt
```

Failure paths need `RC_SOUND` and are read from `stderr.log`, where notifications must still
appear in `notifications.txt`:

```sh
RC_SOUND=/nonexistent.wav drive.sh --name sound-missing \
  --status Discharging --percent 12 --seconds 6 \
  -- --below 20 --no-above --sec 1 --notify-attempts 2

printf 'not audio' > /tmp/bad.wav
RC_SOUND=/tmp/bad.wav drive.sh --name sound-undecodable \
  --status Discharging --percent 12 --seconds 6 \
  -- --below 20 --no-above --sec 1 --notify-attempts 2
```

## Gotchas

- `OutputStream::try_default().unwrap()` panics if no audio device exists, unlike every other
  failure here which is handled. On a machine with no sound server the daemon dies rather than
  degrading. `doctor.sh` warns about this; do not read the resulting empty evidence as a pass.
- The stream is tagged per run, so `audio-streams.txt` is specific to this instance. Without the
  tag the user's own daemon also shows up as `rustcharge` and the count is meaningless.
- One sink-input per notification is the expectation. Fewer means sounds were dropped even though
  notifications went out.
- Shortening or removing the one-second sleep would cut playback off mid-sound. `audio-streams.txt`
  still shows a stream, so the count alone cannot catch it. Listen, or check the stream's duration.
