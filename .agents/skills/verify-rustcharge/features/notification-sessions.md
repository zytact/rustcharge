# Notification session limit

Crossing a threshold opens a session. The session sends at most `--notify-attempts` notifications
(default 15) and then closes. It will not reopen for the same threshold until the battery returns
to the safe zone, which is what stops the daemon nagging forever at 19%
(`NotificationSession`, `src/main.rs:65-114`).

## Sub-features

- A session sends exactly `--notify-attempts` notifications, then stops.
- After a session closes, the same threshold does not reopen it while the battery stays there.
- Entering the safe zone clears the record of the last closed session.
- After the safe zone, crossing the same threshold again opens a fresh session.
- Entering the safe zone mid-session ends that session early.

## How to get to it (user POV)

Let the battery drop below the threshold. The user gets a burst of notifications, then silence
even though the battery is still low. Charging back above the threshold and draining again gives a
new burst.

## Driving it with drive.sh

The full cycle in one run. Expect exactly four notifications: two, silence, then two more.

```sh
drive.sh --name session-cycle --status Discharging --percent 12 --seconds 18 \
  --mutate 8:Discharging:60 --mutate 11:Discharging:12 \
  -- --below 20 --no-above --sec 1 --notify-attempts 2
```

Read `battery-timeline.log` alongside `notifications.txt`: the timeline shows the safe-zone visit
at t+8s and the return to 12% at t+11s, and the notification count shows the session reopened only
after that visit.

Early termination, where the battery recovers before the budget is spent:

```sh
# 10 attempts allowed, but recovery at t+5s ends it after about three.
drive.sh --name session-early-end --status Discharging --percent 12 --seconds 9 \
  --mutate 5:Discharging:60 \
  -- --below 20 --no-above --sec 1 --notify-attempts 10
```

No reopen without a safe-zone visit. Expect exactly two notifications across the whole run:

```sh
drive.sh --name session-no-reopen --status Discharging --percent 12 --seconds 14 \
  -- --below 20 --no-above --sec 1 --notify-attempts 2
```

## Gotchas

- Notification counts are timing-dependent, not exact, whenever the budget is larger than the run
  can spend. Each cycle costs roughly `--sec` plus the one-second sleep in `play_sound`. Assert on
  exact counts only when the budget is the binding limit, as in `session-cycle`.
- The mutation timestamps are wall-clock offsets from launch and are honest about it in
  `battery-timeline.log`. If a mutation lands inside a `play_sound` sleep it takes effect on the
  next poll. Leave a few seconds of slack between mutations.
- Both `--mutate` steps matter. Dropping the safe-zone visit changes the expected count from four
  to two, which is the whole point of the feature.
