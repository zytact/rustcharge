use battery::State;
use clap::{Parser, arg};
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

    #[arg(long, value_parser = clap::value_parser!(u8).range(0..=2), default_value = "1", help = "Notification urgency (0=Low, 1=Normal, 2=Critical)")]
    urgency: u8,

    #[arg(long, value_parser = clap::value_parser!(u8).range(0..=100), default_value = "85", help = "Percentage above which you are notified")]
    above: u8,

    #[arg(long,value_parser = clap::value_parser!(u8).range(0..=100), default_value = "20", help = "Percentage below which you are notified")]
    below: u8,
}

fn main() {
    let args = Cli::parse();
    loop {
        match get_battery_status() {
            Ok((state, ratio)) => {
                let is_charging = match state {
                    State::Charging => true,
                    State::Discharging => false,
                    _ => false,
                };

                let charging_percentage = ratio.value * 100.0;

                let status_text = if is_charging {
                    "Charging"
                } else {
                    "Discharging"
                };

                let urgency = match args.urgency {
                    0 => Urgency::Low,
                    1 => Urgency::Normal,
                    2 => Urgency::Critical,
                    _ => Urgency::Normal,
                };

                let show_notification = |summary: &str, body: &str| {
                    Notification::new()
                        .summary(summary)
                        .body(body)
                        .appname("Rustcharge")
                        .urgency(urgency)
                        .show()
                        .expect("Failed to show notification");

                    play_sound(&args.path);
                };

                if is_charging && charging_percentage >= args.above as f32 {
                    let summary = format!("Battery Status: {}", status_text);
                    let body = format!("Charge: {:.0}%", charging_percentage);
                    show_notification(&summary, &body);
                }

                if !is_charging && charging_percentage <= args.below as f32 {
                    let summary = format!("Battery Status: {}", status_text);
                    let body = format!("Charge: {:.0}%", charging_percentage);
                    show_notification(&summary, &body);
                }
            }
            Err(e) => {
                eprintln!("Error getting battery status: {}", e);
            }
        }
        thread::sleep(Duration::from_secs(120));
    }
}
