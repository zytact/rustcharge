# Rustcharge

RustCharge is a simple, efficient battery status monitor written in Rust. It provides desktop notifications for when your battery goes above or below a certain percentage.

## Features

-   Real-time battery status monitoring
-   Desktop notifications for critical battery levels
-   Custom notification sounds
-   Configurable notification urgency levels
-   Low system resource usage
-   Cross-platform support

## Installation

### Prerequisites

-   Rust and Cargo installed on your system
-   Audio libraries for your platform (ALSA on Linux, CoreAudio on macOS, etc.)

### Building from Source

1. Clone the repository:

    ```sh
    git clone https://github.com/zytact/rustcharge.git
    cd rustcharge
    ```

2. Build the project:

    ```sh
    cargo build --release
    ```

3. The executable will be available at `target/release/rustcharge`

## Usage

```sh
rustcharge --sound-path <sound-file-path> [OPTIONS]
```

### Arguments

-   `--sound-path <path>`: Path to the sound file to play for notifications
-   `--urgency <level>`: Notification urgency (0=Low, 1=Normal, 2=Critical), defaults to 1
-   `--above <percentage>`: Percentage above which you are notified (default: 85)
-   `--below <percentage>`: Percentage below which you are notified (default: 20)
-   `--no-below`: Disable notifications for low battery
-   `--no-above`: Disable notifications for high battery
-   `--sec <seconds>`: Seconds to wait before checking again (default: 120)
-   `--notify-attempts <count>`: Number of notifications per alert session (default: 15, minimum: 1)

### Example

```sh
rustcharge --sound-path /path/to/notification-sound.mp3 --urgency 2 --above 90 --below 15
```

## How It Works

RustCharge uses the `battery` crate to periodically check your battery status. When the battery crosses a threshold (above or below configured levels), it starts a notification session and sends alerts up to the configured `--notify-attempts` limit. Once the notification limit is reached, the session ends and no further notifications are sent until the battery crosses a threshold again, starting a new session.

The monitoring runs in a loop, checking battery status at the interval specified by `--sec` (default: 120 seconds).

## Runtime settings

While the monitor is running, use `set` to change its settings and `status` to read the effective configuration:

```sh
rustcharge set above-enabled false
rustcharge set below-enabled false
rustcharge set above-enabled true
rustcharge set above 90
rustcharge status
```

The available settings are `above`, `below`, `above-enabled`, `below-enabled`, `sound-path`, `sec`, and `notify-attempts`. Linux also supports `urgency`. Percentages must be from 0 to 100, urgency must be from 0 to 2, and notification attempts must be at least 1.

`set` and `status` connect to an existing monitor. They do not start one. A command wakes the monitor without waiting for the polling interval, and a successful `set` writes the change to the per-user config file before applying it. Rustcharge stores configuration in `%APPDATA%\rustcharge\config.toml` on Windows, `$XDG_CONFIG_HOME/rustcharge/config.toml` when `XDG_CONFIG_HOME` is set, or `~/.config/rustcharge/config.toml` otherwise.

Settings use this precedence, from lowest to highest: built-in defaults, persisted settings, and explicit flags passed when the monitor starts. A runtime `set` command replaces that setting for the running monitor and persists it. Other explicit startup flags remain process-only and are not copied into the config file.

Disabling a threshold ends its current alert session and discards any queued sound for that threshold. A sound that has already started can finish. Enabling the threshold allows a fresh evaluation. Changing sound or urgency keeps the current session's notification attempt count.

The control listener accepts authenticated connections only on the local machine. On Linux and macOS, Rustcharge restricts its control files to the current user. On Windows, the files inherit the access controls of the user's AppData directory.

## Platform Support

-   **Linux**: Full support
-   **macOS**: Supported
-   **Windows**: Basic support (notifications may have limited functionality)

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
