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

## Platform Support

-   **Linux**: Full support
-   **macOS**: Supported
-   **Windows**: Basic support (notifications may have limited functionality)

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
