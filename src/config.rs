use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoredConfig {
    pub sound_path: Option<String>,
    pub urgency: Option<u8>,
    pub above: Option<u8>,
    pub below: Option<u8>,
    pub above_enabled: Option<bool>,
    pub below_enabled: Option<bool>,
    pub sec: Option<u64>,
    pub notify_attempts: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeConfig {
    pub sound_path: String,
    pub urgency: u8,
    pub above: u8,
    pub below: u8,
    pub above_enabled: bool,
    pub below_enabled: bool,
    pub sec: u64,
    pub notify_attempts: u64,
}

impl StoredConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.sound_path.as_deref() == Some("") {
            return Err("sound-path cannot be empty".to_string());
        }
        validate_optional_range("above", self.above, 0, 100)?;
        validate_optional_range("below", self.below, 0, 100)?;
        validate_optional_range("urgency", self.urgency, 0, 2)?;
        if self.notify_attempts == Some(0) {
            return Err("notify-attempts must be at least 1".to_string());
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        match fs::read_to_string(path) {
            Ok(contents) => toml::from_str(&contents)
                .map_err(|error| format!("Failed to parse {}: {error}", path.display())),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("Failed to read {}: {error}", path.display())),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let parent = path
            .parent()
            .ok_or_else(|| format!("Invalid config path: {}", path.display()))?;
        create_private_dir(parent)?;
        let contents = toml::to_string_pretty(self)
            .map_err(|error| format!("Failed to serialize config: {error}"))?;
        let mut file = open_private_atomic(path)
            .map_err(|error| format!("Failed to write {}: {error}", path.display()))?;
        file.write_all(contents.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("Failed to write {}: {error}", path.display()))?;
        file.commit()
            .map_err(|error| format!("Failed to replace {}: {error}", path.display()))
    }

    pub fn set(&mut self, key: SettingKey, value: &str) -> Result<SettingValue, String> {
        let parsed = key.parse_value(value)?;
        match &parsed {
            SettingValue::Bool(value) => match key {
                SettingKey::AboveEnabled => self.above_enabled = Some(*value),
                SettingKey::BelowEnabled => self.below_enabled = Some(*value),
                _ => unreachable!(),
            },
            SettingValue::U8(value) => match key {
                SettingKey::Above => self.above = Some(*value),
                SettingKey::Below => self.below = Some(*value),
                SettingKey::Urgency => self.urgency = Some(*value),
                _ => unreachable!(),
            },
            SettingValue::U64(value) => match key {
                SettingKey::Sec => self.sec = Some(*value),
                SettingKey::NotifyAttempts => self.notify_attempts = Some(*value),
                _ => unreachable!(),
            },
            SettingValue::String(value) => match key {
                SettingKey::SoundPath => self.sound_path = Some(value.clone()),
                _ => unreachable!(),
            },
        }
        Ok(parsed)
    }
}

impl RuntimeConfig {
    pub fn defaults(sound_path: String) -> Self {
        Self {
            sound_path,
            urgency: 1,
            above: 85,
            below: 20,
            above_enabled: true,
            below_enabled: true,
            sec: 120,
            notify_attempts: 15,
        }
    }

    pub fn apply_stored(&mut self, stored: &StoredConfig) {
        if let Some(value) = &stored.sound_path {
            self.sound_path.clone_from(value);
        }
        if let Some(value) = stored.urgency {
            self.urgency = value;
        }
        if let Some(value) = stored.above {
            self.above = value;
        }
        if let Some(value) = stored.below {
            self.below = value;
        }
        if let Some(value) = stored.above_enabled {
            self.above_enabled = value;
        }
        if let Some(value) = stored.below_enabled {
            self.below_enabled = value;
        }
        if let Some(value) = stored.sec {
            self.sec = value;
        }
        if let Some(value) = stored.notify_attempts {
            self.notify_attempts = value;
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.sound_path.is_empty() {
            return Err("sound-path cannot be empty".to_string());
        }
        validate_optional_range("above", Some(self.above), 0, 100)?;
        validate_optional_range("below", Some(self.below), 0, 100)?;
        validate_optional_range("urgency", Some(self.urgency), 0, 2)?;
        if self.notify_attempts == 0 {
            return Err("notify-attempts must be at least 1".to_string());
        }
        Ok(())
    }

    pub fn apply_setting(&mut self, key: SettingKey, value: SettingValue) {
        match (key, value) {
            (SettingKey::AboveEnabled, SettingValue::Bool(value)) => self.above_enabled = value,
            (SettingKey::BelowEnabled, SettingValue::Bool(value)) => self.below_enabled = value,
            (SettingKey::Above, SettingValue::U8(value)) => self.above = value,
            (SettingKey::Below, SettingValue::U8(value)) => self.below = value,
            (SettingKey::Urgency, SettingValue::U8(value)) => self.urgency = value,
            (SettingKey::Sec, SettingValue::U64(value)) => self.sec = value,
            (SettingKey::NotifyAttempts, SettingValue::U64(value)) => self.notify_attempts = value,
            (SettingKey::SoundPath, SettingValue::String(value)) => self.sound_path = value,
            _ => unreachable!(),
        }
    }

    pub fn status(&self) -> String {
        format!(
            "above={}\nbelow={}\nabove-enabled={}\nbelow-enabled={}\nsound-path={}\nurgency={}\nsec={}\nnotify-attempts={}",
            self.above,
            self.below,
            self.above_enabled,
            self.below_enabled,
            self.sound_path,
            self.urgency,
            self.sec,
            self.notify_attempts
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingKey {
    Above,
    Below,
    AboveEnabled,
    BelowEnabled,
    SoundPath,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Urgency,
    Sec,
    NotifyAttempts,
}

impl SettingKey {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "above" => Ok(Self::Above),
            "below" => Ok(Self::Below),
            "above-enabled" => Ok(Self::AboveEnabled),
            "below-enabled" => Ok(Self::BelowEnabled),
            "sound-path" => Ok(Self::SoundPath),
            #[cfg(target_os = "linux")]
            "urgency" => Ok(Self::Urgency),
            #[cfg(not(target_os = "linux"))]
            "urgency" => Err("urgency is only available on Linux".to_string()),
            "sec" => Ok(Self::Sec),
            "notify-attempts" => Ok(Self::NotifyAttempts),
            _ => Err(format!("Unknown setting: {value}")),
        }
    }

    fn parse_value(self, value: &str) -> Result<SettingValue, String> {
        match self {
            Self::AboveEnabled | Self::BelowEnabled => value
                .parse::<bool>()
                .map(SettingValue::Bool)
                .map_err(|_| format!("{value:?} is not true or false")),
            Self::Above | Self::Below => parse_in_range(value, 0, 100).map(SettingValue::U8),
            Self::Urgency => parse_in_range(value, 0, 2).map(SettingValue::U8),
            Self::Sec => value
                .parse::<u64>()
                .map(SettingValue::U64)
                .map_err(|_| format!("{value:?} is not a valid number of seconds")),
            Self::NotifyAttempts => {
                let attempts = value
                    .parse::<u64>()
                    .map_err(|_| format!("{value:?} is not a valid attempt count"))?;
                if attempts == 0 {
                    Err("notify-attempts must be at least 1".to_string())
                } else {
                    Ok(SettingValue::U64(attempts))
                }
            }
            Self::SoundPath if value.is_empty() => Err("sound-path cannot be empty".to_string()),
            Self::SoundPath => Ok(SettingValue::String(value.to_string())),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SettingValue {
    Bool(bool),
    U8(u8),
    U64(u64),
    String(String),
}

fn parse_in_range(value: &str, minimum: u8, maximum: u8) -> Result<u8, String> {
    let parsed = value
        .parse::<u8>()
        .map_err(|_| format!("{value:?} is not a number from {minimum} to {maximum}"))?;
    if (minimum..=maximum).contains(&parsed) {
        Ok(parsed)
    } else {
        Err(format!("{parsed} is not from {minimum} to {maximum}"))
    }
}

fn validate_optional_range(
    name: &str,
    value: Option<u8>,
    minimum: u8,
    maximum: u8,
) -> Result<(), String> {
    if let Some(value) = value
        && !(minimum..=maximum).contains(&value)
    {
        return Err(format!("{name} must be from {minimum} to {maximum}"));
    }
    Ok(())
}

pub fn app_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    let base = env::var_os("APPDATA").map(PathBuf::from);

    #[cfg(not(target_os = "windows"))]
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));

    base.map(|path| path.join("rustcharge"))
        .ok_or_else(|| "Could not determine the user config directory".to_string())
}

pub fn create_private_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("Failed to create {}: {error}", path.display()))?;
    set_private_dir_permissions(path)
        .map_err(|error| format!("Failed to protect {}: {error}", path.display()))
}

#[cfg(unix)]
fn set_private_dir_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn set_private_dir_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
pub fn set_private_file_mode(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
}

#[cfg(not(unix))]
pub fn set_private_file_mode(_options: &mut OpenOptions) {}

#[cfg(unix)]
fn open_private_atomic(path: &Path) -> io::Result<AtomicWriteFile> {
    use atomic_write_file::unix::OpenOptionsExt as AtomicOpenOptionsExt;
    use std::os::unix::fs::OpenOptionsExt as StdOpenOptionsExt;
    let mut options = atomic_write_file::OpenOptions::new();
    AtomicOpenOptionsExt::preserve_mode(&mut options, false);
    StdOpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

#[cfg(not(unix))]
fn open_private_atomic(path: &Path) -> io::Result<AtomicWriteFile> {
    AtomicWriteFile::open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_validation_covers_runtime_ranges() {
        assert_eq!(
            SettingKey::Above.parse_value("100"),
            Ok(SettingValue::U8(100))
        );
        assert!(SettingKey::Below.parse_value("101").is_err());
        assert!(SettingKey::Urgency.parse_value("3").is_err());
        assert!(SettingKey::NotifyAttempts.parse_value("0").is_err());
        assert!(SettingKey::AboveEnabled.parse_value("yes").is_err());
    }

    #[test]
    fn stored_config_only_replaces_present_values() {
        let mut runtime = RuntimeConfig::defaults("cli.wav".to_string());
        runtime.apply_stored(&StoredConfig {
            above: Some(90),
            below_enabled: Some(false),
            ..StoredConfig::default()
        });
        assert_eq!(runtime.sound_path, "cli.wav");
        assert_eq!(runtime.above, 90);
        assert!(!runtime.below_enabled);
    }
}
