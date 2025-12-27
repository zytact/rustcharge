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
    last_ended_session: SessionType,
}

impl NotificationSession {
    fn new() -> Self {
        Self {
            session_type: SessionType::None,
            attempts_made: 0,
            last_ended_session: SessionType::None,
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
        self.last_ended_session = self.session_type;
        self.session_type = SessionType::None;
        self.attempts_made = 0;
    }

    fn can_start_session(&self, session_type: SessionType) -> bool {
        // Can start a new session if:
        // 1. No session is active, AND
        // 2. Either this is a different threshold type than the last ended one,
        //    OR no session has ended yet
        !self.is_active() && self.last_ended_session != session_type
    }

    fn clear_last_ended(&mut self) {
        self.last_ended_session = SessionType::None;
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

                // Determine current battery condition for starting new sessions
                let above_threshold =
                    !args.no_above && is_charging && charging_percentage >= args.above as f32;
                let below_threshold =
                    !args.no_below && !is_charging && charging_percentage <= args.below as f32;

                // Handle battery entering safe zone
                if !above_threshold && !below_threshold {
                    // If there's an active session, terminate it
                    if session.is_active() {
                        session.end_session();
                    }
                    // Clear last_ended_session flag when battery is in safe zone
                    else if session.last_ended_session != SessionType::None {
                        session.clear_last_ended();
                    }
                }

                // Session state machine
                match session.session_type {
                    SessionType::None => {
                        // Not in a session - check if we should start one
                        if above_threshold && session.can_start_session(SessionType::AboveThreshold)
                        {
                            session.start_session(SessionType::AboveThreshold);
                        } else if below_threshold
                            && session.can_start_session(SessionType::BelowThreshold)
                        {
                            session.start_session(SessionType::BelowThreshold);
                        }
                    }
                    SessionType::AboveThreshold | SessionType::BelowThreshold => {
                        // In an active session - keep it active until attempts are exhausted
                        // Session will be ended when notify_attempts limit is reached (see below)
                    }
                }

                // Send notification if session is active and attempts remain
                if session.is_active() && session.should_notify(args.notify_attempts) {
                    // Check if current conditions match the session type
                    // For active sessions, we check conditions that match the session
                    let should_send = match session.session_type {
                        SessionType::AboveThreshold => {
                            // Only check if above threshold and charging (original condition)
                            above_threshold
                        }
                        SessionType::BelowThreshold => {
                            // Only check if below threshold and not charging (original condition)
                            below_threshold
                        }
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
