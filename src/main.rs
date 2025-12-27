use battery::State;
use clap::Parser;
use notify_rust::{Notification, Urgency};
use std::thread;
use std::time::Duration;
use utils::battery_status::get_battery_status;
use utils::sound::play_sound;
mod utils;

#[derive(Parser)]
struct Cli {
    #[arg(
        long = "sound-path",
        help = "Path to the sound file to play for notifications"
    )]
    path: String,

    #[cfg(target_os = "linux")]
    #[arg(long, value_parser = clap::value_parser!(u8).range(0..=2), default_value = "1", help = "Notification urgency (0=Low, 1=Normal, 2=Critical)")]
    urgency: u8,

    #[arg(long, value_parser = clap::value_parser!(u8).range(0..=100), default_value = "85", help = "Percentage above which you are notified")]
    above: u8,

    #[arg(long,value_parser = clap::value_parser!(u8).range(0..=100), default_value = "20", help = "Percentage below which you are notified")]
    below: u8,

    #[arg(
        long = "no-below",
        help = "Disable notifications for low battery",
        action = clap::ArgAction::SetTrue
    )]
    no_below: bool,

    #[arg(
        long = "no-above",
        help = "Disable notifications for high battery",
        action = clap::ArgAction::SetTrue
    )]
    no_above: bool,

    #[arg(
        long,
        default_value = "120",
        help = "Seconds to wait before checking again"
    )]
    sec: u64,

    #[arg(
        long = "notify-attempts",
        help = "How many notification attempts per session (minimum 1)",
        value_parser = clap::value_parser!(u64).range(1..),
        default_value = "15"
    )]
    notify_attempts: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SessionType {
    None,
    AboveThreshold,
    BelowThreshold,
}

struct NotificationSession {
    session_type: SessionType,
    attempts_made: u64,
}

impl NotificationSession {
    fn new() -> Self {
        Self {
            session_type: SessionType::None,
            attempts_made: 0,
        }
    }

    fn is_active(&self) -> bool {
        self.session_type != SessionType::None
    }

    fn should_notify(&self, max_attempts: u64) -> bool {
        self.attempts_made < max_attempts
    }

    fn start_session(&mut self, session_type: SessionType) {
        self.session_type = session_type;
        self.attempts_made = 0;
    }

    fn increment_attempt(&mut self) {
        self.attempts_made += 1;
    }

    fn end_session(&mut self) {
        self.session_type = SessionType::None;
        self.attempts_made = 0;
    }
}

fn show_notification(args: &Cli, summary: &str, body: &str) {
    #[cfg(target_os = "linux")]
    let urgency = match args.urgency {
        0 => Urgency::Low,
        1 => Urgency::Normal,
        2 => Urgency::Critical,
        _ => Urgency::Normal,
    };

    #[cfg(target_os = "linux")]
    {
        Notification::new()
            .summary(summary)
            .body(body)
            .appname("Rustcharge")
            .urgency(urgency)
            .show()
            .expect("Failed to show notification");
    }

    #[cfg(not(target_os = "linux"))]
    {
        Notification::new()
            .summary(summary)
            .body(body)
            .appname("Rustcharge")
            .show()
            .expect("Failed to show notification");
    }

    play_sound(&args.path);
}

fn main() {
    let args = Cli::parse();
    let mut session = NotificationSession::new();

    loop {
        match get_battery_status() {
            Ok((state, ratio)) => {
                let is_charging = matches!(state, State::Charging);
                let charging_percentage = ratio.value * 100.0;
                let status_text = if is_charging {
                    "Charging"
                } else {
                    "Discharging"
                };

                // Determine current battery condition
                let above_threshold =
                    !args.no_above && is_charging && charging_percentage >= args.above as f32;
                let below_threshold =
                    !args.no_below && !is_charging && charging_percentage <= args.below as f32;

                // Session state machine
                match session.session_type {
                    SessionType::None => {
                        // Not in a session - check if we should start one
                        if above_threshold {
                            session.start_session(SessionType::AboveThreshold);
                        } else if below_threshold {
                            session.start_session(SessionType::BelowThreshold);
                        }
                    }
                    SessionType::AboveThreshold => {
                        // If battery is no longer above threshold, end the session
                        if !above_threshold {
                            session.end_session();
                        }
                    }
                    SessionType::BelowThreshold => {
                        // If battery is no longer below threshold, end the session
                        if !below_threshold {
                            session.end_session();
                        }
                    }
                }

                // Send notification if session is active and attempts remain
                if session.is_active() && session.should_notify(args.notify_attempts) {
                    let should_send = match session.session_type {
                        SessionType::AboveThreshold => above_threshold,
                        SessionType::BelowThreshold => below_threshold,
                        SessionType::None => false,
                    };

                    if should_send {
                        let summary = format!("Battery Status: {}", status_text);
                        let body = format!("Charge: {:.0}%", charging_percentage);

                        show_notification(&args, &summary, &body);

                        session.increment_attempt();

                        // Check if session should end after this notification
                        if !session.should_notify(args.notify_attempts) {
                            session.end_session();
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error getting battery status: {}", e);
            }
        }
        thread::sleep(Duration::from_secs(args.sec));
    }
}
