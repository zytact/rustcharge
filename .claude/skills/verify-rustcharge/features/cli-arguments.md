# CLI arguments

clap parses and validates every argument before the loop starts, so bad input exits immediately
with a message instead of failing later (`Cli`, `src/main.rs:10-56`).

## Sub-features

- `--sound-path` is required; everything else has a default.
- `--urgency` accepts 0 to 2, `--above` and `--below` accept 0 to 100, `--notify-attempts` accepts
  1 and up. Out-of-range values are rejected with the range in the message.
- Defaults: urgency 1, above 85, below 20, sec 120, notify-attempts 15.
- `--no-above` and `--no-below` are flags taking no value.
- `--help` lists every argument with its help text and default.
- `--urgency` exists only on Linux.

## How to get to it (user POV)

Running `rustcharge` with no arguments, or a typo'd value, and reading what it says.

## Driving it with drive.sh

Not through `drive.sh`. clap exits before the polling loop, so no battery or bus is involved. Run
the binary directly and capture the output:

```sh
out=verification-evidence/$(date -u +%Y%m%dT%H%M%SZ)-cli
mkdir -p "$out"
B=target/release/rustcharge

{
  echo "### no args (expect: required --sound-path)";       "$B"; echo "exit=$?"
  echo "### urgency 3 (expect: not in 0..=2)";              "$B" --sound-path /tmp/x.wav --urgency 3; echo "exit=$?"
  echo "### below 101 (expect: not in 0..=100)";            "$B" --sound-path /tmp/x.wav --below 101; echo "exit=$?"
  echo "### notify-attempts 0 (expect: not in 1..)";        "$B" --sound-path /tmp/x.wav --notify-attempts 0; echo "exit=$?"
  echo "### help (expect: every flag with its default)";    "$B" --help; echo "exit=$?"
} > "$out/cli.txt" 2>&1

cat "$out/cli.txt"
```

Defaults are verified against the running app rather than the help text, since `--help` prints the
declared default whether or not the loop honors it. Drive the boundary implied by each default:

```sh
# Default --below is 20, so 20 fires with no --below passed.
drive.sh --name cli-default-below --status Discharging --percent 20 --seconds 6 \
  -- --no-above --sec 1 --notify-attempts 2

# Default --above is 85, so 85 fires with no --above passed.
drive.sh --name cli-default-above --status Charging --percent 85 --seconds 6 \
  -- --no-below --sec 1 --notify-attempts 2
```

## Gotchas

- There is no `--version`. `doctor.sh` reads the version from `Cargo.toml` and checks the binary is
  newer than `src/`, because a stale binary is the easiest way to verify the wrong code.
- Every argument change has to reach `README.md` too. The repo treats CLI compatibility and the
  README as one thing, so a change that lands here and not there is incomplete.
- `--notify-attempts 0` rejects with `not in 1..18446744073709551615`. The ugly upper bound is
  clap rendering `u64::MAX` and is expected output, not a bug.
