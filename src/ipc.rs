use crate::config::{SettingKey, create_private_dir, set_private_file_mode};
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

const MAX_REQUEST_BYTES: u64 = 8192;
const IO_TIMEOUT: Duration = Duration::from_secs(2);

pub enum ControlCommand {
    Status,
    Set { key: SettingKey, value: String },
}

pub struct ControlRequest {
    pub command: ControlCommand,
    pub reply: Sender<Result<String, String>>,
}

pub struct Server {
    endpoint_path: PathBuf,
    endpoint_contents: String,
}

impl Server {
    pub fn start(app_dir: &Path, sender: Sender<ControlRequest>) -> Result<Self, String> {
        create_private_dir(app_dir)?;
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .map_err(|error| format!("Failed to start control listener: {error}"))?;
        let port = listener
            .local_addr()
            .map_err(|error| format!("Failed to read control address: {error}"))?
            .port();
        let token = generate_token()?;
        let endpoint_contents = format!("{port} {token}\n");
        let endpoint_path = app_dir.join("daemon.endpoint");
        claim_endpoint(&endpoint_path, &endpoint_contents)?;

        let server_token = token;
        thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => handle_connection(stream, &server_token, &sender),
                    Err(error) => eprintln!("Control listener error: {error}"),
                }
            }
        });

        Ok(Self {
            endpoint_path,
            endpoint_contents,
        })
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if fs::read_to_string(&self.endpoint_path)
            .is_ok_and(|contents| contents == self.endpoint_contents)
        {
            let _ = fs::remove_file(&self.endpoint_path);
        }
    }
}

pub fn request(app_dir: &Path, command: &str) -> Result<String, String> {
    let endpoint_path = app_dir.join("daemon.endpoint");
    let endpoint = fs::read_to_string(&endpoint_path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            "rustcharge is not running".to_string()
        } else {
            format!("Failed to read {}: {error}", endpoint_path.display())
        }
    })?;
    request_endpoint(&endpoint, command)
}

fn request_endpoint(endpoint: &str, command: &str) -> Result<String, String> {
    let (port, token) = parse_endpoint(endpoint)?;
    let mut stream = TcpStream::connect_timeout(
        &SocketAddrV4::new(Ipv4Addr::LOCALHOST, port).into(),
        IO_TIMEOUT,
    )
    .map_err(|_| "rustcharge is not running".to_string())?;
    set_timeouts(&stream)
        .map_err(|error| format!("Failed to configure control request: {error}"))?;
    writeln!(stream, "{token} {command}")
        .map_err(|error| format!("Failed to send control request: {error}"))?;
    let mut response = String::new();
    stream
        .take(MAX_REQUEST_BYTES)
        .read_to_string(&mut response)
        .map_err(|error| format!("Failed to read control response: {error}"))?;
    if let Some(body) = response.strip_prefix("OK\n") {
        Ok(body.trim_end().to_string())
    } else if let Some(body) = response.strip_prefix("ERR\n") {
        Err(body.trim_end().to_string())
    } else {
        Err("Invalid response from rustcharge".to_string())
    }
}

fn handle_connection(mut stream: TcpStream, token: &str, sender: &Sender<ControlRequest>) {
    if set_timeouts(&stream).is_err() {
        return;
    }
    let mut line = String::new();
    let read_result = BufReader::new(
        stream
            .try_clone()
            .expect("TCP stream cloning should succeed"),
    )
    .take(MAX_REQUEST_BYTES)
    .read_line(&mut line);
    let response = match read_result {
        Ok(0) => Err("Empty control request".to_string()),
        Ok(_) if !line.ends_with('\n') => Err("Control request is too large".to_string()),
        Err(error) => Err(format!("Failed to read control request: {error}")),
        Ok(_) => parse_request(line.trim_end(), token).and_then(|command| {
            let (reply, response) = mpsc::channel();
            sender
                .send(ControlRequest { command, reply })
                .map_err(|_| "rustcharge monitor stopped".to_string())?;
            response
                .recv_timeout(IO_TIMEOUT)
                .map_err(|_| "rustcharge monitor did not respond".to_string())?
        }),
    };
    let _ = match response {
        Ok(body) => writeln!(stream, "OK\n{body}"),
        Err(error) => writeln!(stream, "ERR\n{error}"),
    };
}

fn parse_request(line: &str, expected_token: &str) -> Result<ControlCommand, String> {
    let (token, command) = line
        .split_once(' ')
        .ok_or_else(|| "Invalid control request".to_string())?;
    if token != expected_token {
        return Err("Unauthorized control request".to_string());
    }
    if command == "status" {
        return Ok(ControlCommand::Status);
    }
    let arguments = command
        .strip_prefix("set ")
        .ok_or_else(|| "Unknown control command".to_string())?;
    let (key, value) = arguments
        .split_once(' ')
        .ok_or_else(|| "set requires a setting and value".to_string())?;
    Ok(ControlCommand::Set {
        key: SettingKey::parse(key)?,
        value: value.to_string(),
    })
}

fn claim_endpoint(path: &Path, contents: &str) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    set_private_file_mode(&mut options);
    match options.open(path) {
        Ok(mut file) => file
            .write_all(contents.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("Failed to write {}: {error}", path.display())),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let existing = fs::read_to_string(path)
                .map_err(|error| format!("Failed to inspect existing endpoint: {error}"))?;
            if parse_endpoint(&existing).is_err() {
                let age = fs::metadata(path)
                    .and_then(|metadata| metadata.modified())
                    .and_then(|modified| modified.elapsed().map_err(io::Error::other))
                    .map_err(|error| format!("Failed to inspect existing endpoint: {error}"))?;
                if age < IO_TIMEOUT {
                    return Err("Another rustcharge monitor is starting".to_string());
                }
                fs::remove_file(path)
                    .map_err(|error| format!("Failed to remove stale endpoint: {error}"))?;
                return claim_endpoint(path, contents);
            }
            match request_endpoint(&existing, "status") {
                Ok(_) => Err("Another rustcharge monitor is already running".to_string()),
                Err(error) if error == "rustcharge is not running" => {
                    fs::remove_file(path).map_err(|remove_error| {
                        format!("Failed to remove stale endpoint: {remove_error}")
                    })?;
                    claim_endpoint(path, contents)
                }
                Err(error) => Err(format!(
                    "Could not verify the existing rustcharge monitor: {error}"
                )),
            }
        }
        Err(error) => Err(format!("Failed to claim {}: {error}", path.display())),
    }
}

fn parse_endpoint(endpoint: &str) -> Result<(u16, String), String> {
    let (port, token) = endpoint
        .trim()
        .split_once(' ')
        .ok_or_else(|| "Invalid rustcharge endpoint file".to_string())?;
    let port = port
        .parse::<u16>()
        .map_err(|_| "Invalid rustcharge endpoint port".to_string())?;
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Invalid rustcharge endpoint token".to_string());
    }
    Ok((port, token.to_string()))
}

fn generate_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|error| format!("Failed to secure control endpoint: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn set_timeouts(stream: &TcpStream) -> io::Result<()> {
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_requires_token_and_preserves_value_spaces() {
        let request = parse_request("secret set sound-path /tmp/my sound.wav", "secret").unwrap();
        match request {
            ControlCommand::Set { key, value } => {
                assert_eq!(key, SettingKey::SoundPath);
                assert_eq!(value, "/tmp/my sound.wav");
            }
            ControlCommand::Status => panic!("expected set"),
        }
        assert!(parse_request("wrong status", "secret").is_err());
    }
}
