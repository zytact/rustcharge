use battery::State;
use notify_rust::{Hint, Notification};
use std::thread;
use std::time::Duration;
use utils::battery_status::get_battery_status;
mod utils;

fn main() {
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

                let show_notification = |summary: &str, body: &str| {
                    Notification::new()
                        .summary(summary)
                        .body(body)
                        .hint(Hint::SoundName("message".into()))
                        .show()
                        .expect("Failed to show notification");
                };

                if is_charging && charging_percentage >= 85.0 {
                    let summary = format!("Battery Status: {}", status_text);
                    let body = format!("Charge: {:.0}%", charging_percentage);
                    show_notification(&summary, &body);
                }

                if !is_charging && charging_percentage <= 25.0 {
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
