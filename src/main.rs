use battery::State;
use clap::{Args, Parser, Subcommand};
use config::{RuntimeConfig, SettingKey, StoredConfig, app_dir};
use ipc::{ControlCommand, ControlRequest};
use notify_rust::Notification;
#[cfg(target_os = "linux")]
use notify_rust::Urgency;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::thread;
use std::time::{Duration, Instant};
use utils::battery_status::get_battery_status;
use utils::sound::play_sound;

mod config;
mod ipc;
mod utils;

#[derive(Parser)]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    #[command(flatten)]
    monitor: MonitorArgs,
}

#[derive(Subcommand)]
enum Command {
    Set { setting: String, value: String },
    Status,
}

#[derive(Args, Default)]
struct MonitorArgs {
    #[arg(
        long = "sound-path",
        help = "Path to the sound file to play for notifications"
    )]
    path: Option<String>,
    #[cfg(target_os = "linux")]
    #[arg(long, value_parser = clap::value_parser!(u8).range(0..=2), help = "Notification urgency (0=Low, 1=Normal, 2=Critical; default: 1)")]
    urgency: Option<u8>,
    #[arg(long, value_parser = clap::value_parser!(u8).range(0..=100), help = "Percentage above which you are notified (default: 85)")]
    above: Option<u8>,
    #[arg(long, value_parser = clap::value_parser!(u8).range(0..=100), help = "Percentage below which you are notified (default: 20)")]
    below: Option<u8>,
    #[arg(long = "no-below", help = "Disable notifications for low battery", action = clap::ArgAction::SetTrue)]
    no_below: bool,
    #[arg(long = "no-above", help = "Disable notifications for high battery", action = clap::ArgAction::SetTrue)]
    no_above: bool,
    #[arg(long, help = "Seconds to wait before checking again (default: 120)")]
    sec: Option<u64>,
    #[arg(long = "notify-attempts", help = "How many notification attempts per session (default: 15, minimum: 1)", value_parser = clap::value_parser!(u64).range(1..))]
    notify_attempts: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
        !self.is_active() && self.last_ended_session != session_type
    }
    fn clear_last_ended(&mut self) {
        self.last_ended_session = SessionType::None;
    }

    fn setting_changed(&mut self, key: SettingKey, value: &config::SettingValue) {
        let affected = match (key, value) {
            (SettingKey::AboveEnabled, config::SettingValue::Bool(_)) => {
                SessionType::AboveThreshold
            }
            (SettingKey::BelowEnabled, config::SettingValue::Bool(_)) => {
                SessionType::BelowThreshold
            }
            _ => return,
        };
        let config::SettingValue::Bool(enabled) = value else {
            unreachable!()
        };
        if !enabled && self.session_type == affected {
            self.end_session();
        } else if *enabled && self.last_ended_session == affected {
            self.clear_last_ended();
        }
    }
}

struct SoundEpochs {
    above: AtomicU64,
    below: AtomicU64,
}

impl SoundEpochs {
    fn new() -> Self {
        Self {
            above: AtomicU64::new(0),
            below: AtomicU64::new(0),
        }
    }
    fn current(&self, session_type: SessionType) -> u64 {
        match session_type {
            SessionType::AboveThreshold => self.above.load(Ordering::Acquire),
            SessionType::BelowThreshold => self.below.load(Ordering::Acquire),
            SessionType::None => 0,
        }
    }
    fn cancel(&self, session_type: SessionType) {
        match session_type {
            SessionType::AboveThreshold => {
                self.above.fetch_add(1, Ordering::AcqRel);
            }
            SessionType::BelowThreshold => {
                self.below.fetch_add(1, Ordering::AcqRel);
            }
            SessionType::None => {}
        }
    }
}

struct SoundJob {
    path: String,
    session_type: SessionType,
    epoch: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ControlEffect {
    KeepDeadline,
    EvaluateNow,
    ResetDeadline,
}

fn start_sound_worker(epochs: Arc<SoundEpochs>) -> SyncSender<SoundJob> {
    let (sender, receiver) = mpsc::sync_channel::<SoundJob>(1);
    thread::spawn(move || {
        while let Ok(job) = receiver.recv() {
            if epochs.current(job.session_type) == job.epoch {
                play_sound(&job.path);
            }
        }
    });
    sender
}

fn show_notification(
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] config: &RuntimeConfig,
    summary: &str,
    body: &str,
) {
    #[cfg(target_os = "linux")]
    let urgency = match config.urgency {
        0 => Urgency::Low,
        1 => Urgency::Normal,
        2 => Urgency::Critical,
        _ => unreachable!(),
    };
    #[cfg(target_os = "linux")]
    Notification::new()
        .summary(summary)
        .body(body)
        .appname("Rustcharge")
        .urgency(urgency)
        .show()
        .expect("Failed to show notification");
    #[cfg(not(target_os = "linux"))]
    Notification::new()
        .summary(summary)
        .body(body)
        .appname("Rustcharge")
        .show()
        .expect("Failed to show notification");
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    let app_dir = app_dir()?;
    match cli.command {
        Some(Command::Set { setting, value }) => {
            let setting = SettingKey::parse(&setting)?;
            println!(
                "{}",
                ipc::request(&app_dir, &format!("set {} {value}", setting_name(setting)))?
            );
            Ok(())
        }
        Some(Command::Status) => {
            println!("{}", ipc::request(&app_dir, "status")?);
            Ok(())
        }
        None => run_monitor(cli.monitor, app_dir),
    }
}

fn run_monitor(args: MonitorArgs, app_dir: std::path::PathBuf) -> Result<(), String> {
    let config_path = app_dir.join("config.toml");
    let mut stored = StoredConfig::load(&config_path)?;
    stored.validate()?;
    let initial_sound = args
        .path
        .clone()
        .or_else(|| stored.sound_path.clone())
        .ok_or_else(|| {
            "--sound-path is required when no persisted sound-path is set".to_string()
        })?;
    let mut config = RuntimeConfig::defaults(initial_sound);
    config.apply_stored(&stored);
    apply_cli_overrides(&mut config, args);
    config.validate()?;

    let (control_sender, control_receiver) = mpsc::channel();
    let _server = ipc::Server::start(&app_dir, control_sender)?;
    let epochs = Arc::new(SoundEpochs::new());
    let sound_sender = start_sound_worker(Arc::clone(&epochs));
    let mut session = NotificationSession::new();
    evaluate_battery(&config, &mut session, &sound_sender, &epochs);
    let mut next_check = Instant::now() + Duration::from_secs(config.sec);

    loop {
        match control_receiver.recv_timeout(next_check.saturating_duration_since(Instant::now())) {
            Ok(request) => {
                let effect = handle_control(
                    request,
                    &mut stored,
                    &mut config,
                    &mut session,
                    &epochs,
                    &config_path,
                );
                if effect == ControlEffect::EvaluateNow {
                    evaluate_battery(&config, &mut session, &sound_sender, &epochs);
                }
                next_check = update_deadline(effect, next_check, Instant::now(), config.sec);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                evaluate_battery(&config, &mut session, &sound_sender, &epochs);
                next_check = Instant::now() + Duration::from_secs(config.sec);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("Control listener stopped".to_string());
            }
        }
    }
}

fn evaluate_battery(
    config: &RuntimeConfig,
    session: &mut NotificationSession,
    sound_sender: &SyncSender<SoundJob>,
    epochs: &SoundEpochs,
) {
    let Ok((state, ratio)) = get_battery_status()
        .inspect_err(|error| eprintln!("Error getting battery status: {error}"))
    else {
        return;
    };
    let is_charging = matches!(state, State::Charging);
    let percentage = ratio.value * 100.0;
    let (above, below) = battery_conditions(config, is_charging, percentage);

    if !above && !below {
        if session.is_active() {
            session.end_session();
        } else if session.last_ended_session != SessionType::None {
            session.clear_last_ended();
        }
    }
    if session.is_active() && !session.should_notify(config.notify_attempts) {
        session.end_session();
    }
    if !session.is_active() {
        if above && session.can_start_session(SessionType::AboveThreshold) {
            session.start_session(SessionType::AboveThreshold);
        } else if below && session.can_start_session(SessionType::BelowThreshold) {
            session.start_session(SessionType::BelowThreshold);
        }
    }

    let should_send = match session.session_type {
        SessionType::AboveThreshold => above,
        SessionType::BelowThreshold => below,
        SessionType::None => false,
    };
    if should_send && session.should_notify(config.notify_attempts) {
        let status = if is_charging {
            "Charging"
        } else {
            "Discharging"
        };
        show_notification(
            config,
            &format!("Battery Status: {status}"),
            &format!("Charge: {percentage:.0}%"),
        );
        let job = SoundJob {
            path: config.sound_path.clone(),
            session_type: session.session_type,
            epoch: epochs.current(session.session_type),
        };
        if let Err(TrySendError::Full(_)) = sound_sender.try_send(job) {
            eprintln!("Skipped notification sound because audio playback is still busy");
        }
        session.increment_attempt();
        if !session.should_notify(config.notify_attempts) {
            session.end_session();
        }
    }
}

fn battery_conditions(config: &RuntimeConfig, is_charging: bool, percentage: f32) -> (bool, bool) {
    (
        config.above_enabled && is_charging && percentage >= config.above as f32,
        config.below_enabled && !is_charging && percentage <= config.below as f32,
    )
}

fn handle_control(
    request: ControlRequest,
    stored: &mut StoredConfig,
    config: &mut RuntimeConfig,
    session: &mut NotificationSession,
    epochs: &SoundEpochs,
    config_path: &std::path::Path,
) -> ControlEffect {
    let mut effect = ControlEffect::KeepDeadline;
    let response = match request.command {
        ControlCommand::Status => Ok(config.status()),
        ControlCommand::Set { key, value } => {
            let mut next_stored = stored.clone();
            next_stored.set(key, &value).and_then(|parsed| {
                next_stored.validate()?;
                next_stored.save(config_path)?;
                effect = control_effect(config, key, &parsed);
                if effect == ControlEffect::EvaluateNow
                    && matches!(
                        (key, &parsed),
                        (SettingKey::AboveEnabled, config::SettingValue::Bool(false))
                    )
                {
                    epochs.cancel(SessionType::AboveThreshold);
                }
                if effect == ControlEffect::EvaluateNow
                    && matches!(
                        (key, &parsed),
                        (SettingKey::BelowEnabled, config::SettingValue::Bool(false))
                    )
                {
                    epochs.cancel(SessionType::BelowThreshold);
                }
                if effect == ControlEffect::EvaluateNow {
                    session.setting_changed(key, &parsed);
                }
                config.apply_setting(key, parsed);
                *stored = next_stored;
                Ok(config.status())
            })
        }
    };
    let succeeded = response.is_ok();
    let _ = request.reply.send(response);
    if succeeded {
        effect
    } else {
        ControlEffect::KeepDeadline
    }
}

fn control_effect(
    config: &RuntimeConfig,
    key: SettingKey,
    value: &config::SettingValue,
) -> ControlEffect {
    use config::SettingValue;
    let changed = match (key, value) {
        (SettingKey::Above, SettingValue::U8(value)) => config.above != *value,
        (SettingKey::Below, SettingValue::U8(value)) => config.below != *value,
        (SettingKey::AboveEnabled, SettingValue::Bool(value)) => config.above_enabled != *value,
        (SettingKey::BelowEnabled, SettingValue::Bool(value)) => config.below_enabled != *value,
        (SettingKey::SoundPath, SettingValue::String(value)) => config.sound_path != *value,
        (SettingKey::Urgency, SettingValue::U8(value)) => config.urgency != *value,
        (SettingKey::Sec, SettingValue::U64(value)) => config.sec != *value,
        (SettingKey::NotifyAttempts, SettingValue::U64(value)) => config.notify_attempts != *value,
        _ => unreachable!(),
    };
    if !changed {
        return ControlEffect::KeepDeadline;
    }
    match key {
        SettingKey::Above
        | SettingKey::Below
        | SettingKey::AboveEnabled
        | SettingKey::BelowEnabled => ControlEffect::EvaluateNow,
        SettingKey::Sec => ControlEffect::ResetDeadline,
        SettingKey::SoundPath | SettingKey::Urgency | SettingKey::NotifyAttempts => {
            ControlEffect::KeepDeadline
        }
    }
}

fn update_deadline(
    effect: ControlEffect,
    current: Instant,
    now: Instant,
    interval_seconds: u64,
) -> Instant {
    match effect {
        ControlEffect::KeepDeadline => current,
        ControlEffect::EvaluateNow | ControlEffect::ResetDeadline => {
            now + Duration::from_secs(interval_seconds)
        }
    }
}

fn apply_cli_overrides(config: &mut RuntimeConfig, args: MonitorArgs) {
    if let Some(value) = args.path {
        config.sound_path = value;
    }
    #[cfg(target_os = "linux")]
    if let Some(value) = args.urgency {
        config.urgency = value;
    }
    if let Some(value) = args.above {
        config.above = value;
    }
    if let Some(value) = args.below {
        config.below = value;
    }
    if args.no_above {
        config.above_enabled = false;
    }
    if args.no_below {
        config.below_enabled = false;
    }
    if let Some(value) = args.sec {
        config.sec = value;
    }
    if let Some(value) = args.notify_attempts {
        config.notify_attempts = value;
    }
}

fn setting_name(setting: SettingKey) -> &'static str {
    match setting {
        SettingKey::Above => "above",
        SettingKey::Below => "below",
        SettingKey::AboveEnabled => "above-enabled",
        SettingKey::BelowEnabled => "below-enabled",
        SettingKey::SoundPath => "sound-path",
        SettingKey::Urgency => "urgency",
        SettingKey::Sec => "sec",
        SettingKey::NotifyAttempts => "notify-attempts",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabling_and_reenabling_threshold_resets_that_session() {
        let mut session = NotificationSession::new();
        session.start_session(SessionType::AboveThreshold);
        session.increment_attempt();
        session.setting_changed(SettingKey::AboveEnabled, &config::SettingValue::Bool(false));
        assert_eq!(session.session_type, SessionType::None);
        assert_eq!(session.last_ended_session, SessionType::AboveThreshold);
        session.setting_changed(SettingKey::AboveEnabled, &config::SettingValue::Bool(true));
        assert!(session.can_start_session(SessionType::AboveThreshold));
    }

    #[test]
    fn sound_and_urgency_keep_attempt_count() {
        let mut session = NotificationSession::new();
        session.start_session(SessionType::BelowThreshold);
        session.increment_attempt();
        session.setting_changed(
            SettingKey::SoundPath,
            &config::SettingValue::String("new.wav".to_string()),
        );
        session.setting_changed(SettingKey::Urgency, &config::SettingValue::U8(2));
        assert_eq!(session.session_type, SessionType::BelowThreshold);
        assert_eq!(session.attempts_made, 1);
    }

    #[test]
    fn threshold_boundaries_are_inclusive_and_respect_charging_state() {
        let config = RuntimeConfig::defaults("sound.wav".to_string());
        assert_eq!(battery_conditions(&config, true, 85.0), (true, false));
        assert_eq!(battery_conditions(&config, true, 84.9), (false, false));
        assert_eq!(battery_conditions(&config, false, 20.0), (false, true));
        assert_eq!(battery_conditions(&config, false, 20.1), (false, false));
    }

    #[test]
    fn observational_commands_preserve_the_poll_deadline() {
        let now = Instant::now();
        let deadline = now + Duration::from_secs(120);
        let later = now + Duration::from_secs(15);
        assert_eq!(
            update_deadline(ControlEffect::KeepDeadline, deadline, later, 120),
            deadline
        );
    }

    #[test]
    fn interval_and_evaluation_changes_reset_the_poll_deadline() {
        let now = Instant::now();
        let old_deadline = now + Duration::from_secs(120);
        let changed_at = now + Duration::from_secs(10);
        for effect in [ControlEffect::EvaluateNow, ControlEffect::ResetDeadline] {
            assert_eq!(
                update_deadline(effect, old_deadline, changed_at, 30),
                changed_at + Duration::from_secs(30)
            );
        }
    }

    #[test]
    fn only_changed_thresholds_request_immediate_evaluation() {
        let config = RuntimeConfig::defaults("sound.wav".to_string());
        assert_eq!(
            control_effect(&config, SettingKey::Above, &config::SettingValue::U8(90)),
            ControlEffect::EvaluateNow
        );
        assert_eq!(
            control_effect(&config, SettingKey::Above, &config::SettingValue::U8(85)),
            ControlEffect::KeepDeadline
        );
        assert_eq!(
            control_effect(&config, SettingKey::Urgency, &config::SettingValue::U8(2)),
            ControlEffect::KeepDeadline
        );
        assert_eq!(
            control_effect(&config, SettingKey::Sec, &config::SettingValue::U64(30)),
            ControlEffect::ResetDeadline
        );
    }
}
