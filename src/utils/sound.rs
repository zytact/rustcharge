use rodio::{Decoder, OutputStream, Source};
use std::io::BufReader;
use std::thread;
use std::time::Duration;

pub fn play_sound(path: &str) {
    match std::fs::File::open(path) {
        Ok(file) => {
            let (_stream, stream_handle) = OutputStream::try_default().unwrap();
            let file = BufReader::new(file);
            let source = match Decoder::new(file) {
                Ok(src) => src.convert_samples(),
                Err(e) => {
                    eprintln!("Failed to decode audio: {}", e);
                    return;
                }
            };

            if let Err(e) = stream_handle.play_raw(source) {
                eprintln!("Failed to play sound: {}", e);
            }

            thread::sleep(Duration::from_millis(1000)); // Give time for sound to play
        }
        Err(e) => eprintln!("Failed to open sound file: {}", e),
    }
}
