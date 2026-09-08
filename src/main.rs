mod audio;
mod dsp;
mod protocol;

use serde_json::{Value, json};
use std::{
    fs::File,
    io::{self, BufRead, Read, Seek, Write},
    net::{Shutdown, TcpStream, ToSocketAddrs},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

fn event(state: &str, message: &str) {
    println!("{}", json!({"state": state, "message": message}));
}
fn valid_frequency(f: f64) -> bool {
    f.is_finite() && (65_000_000.0..=108_000_000.0).contains(&f)
}
fn frequency(s: &str) -> Result<f64, String> {
    let f: f64 = s
        .replace(',', ".")
        .parse()
        .map_err(|_| "Invalid frequency in MHz")?;
    let f = f * 1e6;
    if !valid_frequency(f) {
        return Err("FM frequency must be between 65 and 108 MHz".into());
    }
    Ok(f)
}

struct Options {
    server: String,
    frequency: f64,
    volume: f32,
    seconds: Option<f64>,
    wav: Option<String>,
    audio: bool,
}
fn options() -> Result<Options, String> {
    let mut opt = Options {
        server: "127.0.0.1:5259".into(),
        frequency: 102_400_000.0,
        volume: 0.3,
        seconds: None,
        wav: None,
        audio: true,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--help" {
            println!(
                "OmaSDR 1.1.0 — stereo broadcast FM for SDR++\n\
                --server HOST:PORT --frequency MHz --volume 0..1\n\
                --seconds N --wav FILE --no-audio\n\
                stdin JSON lines: {{\"frequency\":102.4}}, {{\"volume\":0.3}}, {{\"stop\":true}}\n\
                Closing stdin disconnects. --seconds allows unattended tests."
            );
            std::process::exit(0);
        }
        if a == "--no-audio" {
            opt.audio = false;
            continue;
        }
        let v = args
            .next()
            .ok_or_else(|| format!("Missing value for {a}"))?;
        match a.as_str() {
            "--server" => opt.server = v,
            "--frequency" => opt.frequency = frequency(&v)?,
            "--volume" => {
                opt.volume = v.parse().map_err(|_| "Invalid volume")?;
                if !opt.volume.is_finite() || !(0.0..=1.0).contains(&opt.volume) {
                    return Err("Volume must be between 0 and 1".into());
                }
            }
            "--seconds" => {
                let s: f64 = v.parse().map_err(|_| "Invalid test duration")?;
                if !s.is_finite() || !(0.1..=3600.0).contains(&s) {
                    return Err("Test duration must be 0.1 to 3600 seconds".into());
                }
                opt.seconds = Some(s);
            }
            "--wav" => opt.wav = Some(v),
            _ => return Err(format!("Unknown argument: {a}")),
        }
    }
    Ok(opt)
}

struct Wav {
    file: File,
    bytes: u32,
}
impl Wav {
    fn new(path: &str) -> io::Result<Self> {
        let mut file = File::create_new(path)?;
        file.write_all(&[0; 44])?;
        Ok(Self { file, bytes: 0 })
    }
    fn write(&mut self, audio: &[f32]) -> io::Result<()> {
        if self.bytes as u64 + audio.len() as u64 * 2 > u32::MAX as u64 - 36 {
            return Err(io::Error::other("WAV size limit reached"));
        }
        for &x in audio {
            self.file
                .write_all(&((x.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
        }
        self.bytes += audio.len() as u32 * 2;
        Ok(())
    }
    fn finish(&mut self) -> io::Result<()> {
        self.file.rewind()?;
        self.file.write_all(b"RIFF")?;
        self.file.write_all(&(36 + self.bytes).to_le_bytes())?;
        self.file.write_all(b"WAVEfmt ")?;
        self.file.write_all(&16u32.to_le_bytes())?;
        self.file.write_all(&1u16.to_le_bytes())?;
        self.file.write_all(&2u16.to_le_bytes())?;
        self.file.write_all(&48_000u32.to_le_bytes())?;
        self.file.write_all(&192_000u32.to_le_bytes())?;
        self.file.write_all(&4u16.to_le_bytes())?;
        self.file.write_all(&16u16.to_le_bytes())?;
        self.file.write_all(b"data")?;
        self.file.write_all(&self.bytes.to_le_bytes())?;
        self.file.flush()
    }
}
impl Drop for Wav {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

fn run(opt: Options) -> Result<(), String> {
    event("connecting", &format!("Connecting to {}", opt.server));
    let addresses: Vec<_> = opt
        .server
        .to_socket_addrs()
        .map_err(|e| format!("Invalid server address: {e}"))?
        .collect();
    let mut socket = addresses
        .iter()
        .find_map(|a| TcpStream::connect_timeout(a, Duration::from_secs(3)).ok())
        .ok_or("Cannot connect to SDR++ server; check its address and port")?;
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|e| e.to_string())?;
    socket
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    let _ = socket.set_nodelay(true);
    let stop = Arc::new(AtomicBool::new(false));
    let desired_freq = Arc::new(AtomicU64::new(opt.frequency.to_bits()));
    let volume = Arc::new(AtomicU32::new(opt.volume.to_bits()));
    let stdin_socket = socket.try_clone().map_err(|e| e.to_string())?;
    let (s, f, v) = (stop.clone(), desired_freq.clone(), volume.clone());
    let timed = opt.seconds.is_some();
    std::thread::spawn(move || {
        let mut reader = io::stdin().lock();
        loop {
            let mut line = String::new();
            // A malformed controller cannot cause an unbounded allocation.
            match reader.by_ref().take(4097).read_line(&mut line) {
                Ok(0) if timed => return,
                Ok(0) | Err(_) => break,
                Ok(_) if line.len() > 4096 => break,
                _ => {}
            }
            match serde_json::from_str::<Value>(&line) {
                Ok(cmd) => {
                    if cmd["stop"] == true {
                        break;
                    }
                    if let Some(hz) = cmd["frequency"].as_f64().map(|x| x * 1e6) {
                        if valid_frequency(hz) {
                            f.store(hz.to_bits(), Ordering::Relaxed);
                        } else {
                            event("warning", "FM frequency must be between 65 and 108 MHz");
                        }
                    }
                    if let Some(n) = cmd["volume"].as_f64().filter(|n| (0.0..=1.0).contains(n)) {
                        v.store((n as f32).to_bits(), Ordering::Relaxed);
                    }
                }
                Err(_) => event("warning", "Invalid controller command"),
            }
        }
        s.store(true, Ordering::Relaxed);
        let _ = stdin_socket.shutdown(Shutdown::Read);
    });
    let mut framer = protocol::Framer::default();
    let mut buffer = [0; 65536];
    let mut iq = vec![];
    let mut pcm = vec![];
    let mut fm = None;
    let mut sample_rate = 0.0;
    let mut output = None;
    let mut wav = opt
        .wav
        .as_deref()
        .map(Wav::new)
        .transpose()
        .map_err(|e| e.to_string())?;
    let mut tuned = opt.frequency;
    let started = Instant::now();
    let mut last_frame = started;
    let mut last_iq = started;
    let mut report = started;
    let mut mute_until = started;
    let mut total_audio = 0u64;
    let mut dropped = 0u64;
    let mut playing = false;
    let mut stereo = false;
    let result = (|| -> Result<(), String> {
        loop {
            if stop.load(Ordering::Relaxed)
                || opt
                    .seconds
                    .is_some_and(|s| started.elapsed().as_secs_f64() >= s)
            {
                break;
            }
            if output
                .as_ref()
                .is_some_and(|a: &audio::Audio| a.failed.load(Ordering::Relaxed))
            {
                return Err("Audio service disconnected".into());
            }
            let wanted = f64::from_bits(desired_freq.load(Ordering::Relaxed));
            if wanted != tuned && fm.is_some() {
                protocol::command(&mut socket, 4, &wanted.to_le_bytes())
                    .map_err(|e| e.to_string())?;
                tuned = wanted;
                fm = Some(dsp::Fm::new(sample_rate)?);
                mute_until = Instant::now() + Duration::from_millis(150);
                event("tuning", &format!("Tuning to {:.1} MHz", tuned / 1e6));
                playing = false;
                stereo = false;
            }
            match socket.read(&mut buffer) {
                Ok(0) if stop.load(Ordering::Relaxed) => break,
                Ok(0) => return Err("SDR++ server closed the connection".into()),
                Ok(n) => framer.push(&buffer[..n])?,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock
                            | io::ErrorKind::TimedOut
                            | io::ErrorKind::Interrupted
                    ) => {}
                Err(_) if stop.load(Ordering::Relaxed) => break,
                Err(e) => return Err(format!("Server connection failed: {e}")),
            }
            while let Some((kind, data)) = framer.next()? {
                last_frame = Instant::now();
                match kind {
                    0 | 1 => {
                        if data.len() < 4 {
                            return Err("Truncated server command".into());
                        }
                        let command = u32::from_le_bytes(data[..4].try_into().unwrap());
                        if command == 0x81 {
                            return Err(
                                "SDR++ server is busy; disconnect its other client first".into()
                            );
                        }
                        if command == 0x80 {
                            if data.len() != 12 {
                                return Err("Invalid sample rate message".into());
                            }
                            sample_rate = f64::from_le_bytes(data[4..].try_into().unwrap());
                            let first = fm.is_none();
                            fm = Some(dsp::Fm::new(sample_rate)?);
                            if first {
                                if opt.audio {
                                    output =
                                        Some(audio::Audio::new().map_err(|e| {
                                            format!("Cannot open audio output: {e}")
                                        })?);
                                }
                                protocol::command(&mut socket, 6, &[1])
                                    .map_err(|e| e.to_string())?;
                                protocol::command(&mut socket, 7, &[0])
                                    .map_err(|e| e.to_string())?;
                                protocol::command(&mut socket, 4, &tuned.to_le_bytes())
                                    .map_err(|e| e.to_string())?;
                                protocol::command(&mut socket, 2, &[])
                                    .map_err(|e| e.to_string())?;
                            }
                            event(
                                "receiving",
                                &format!(
                                    "Receiving {:.1} MHz · {:.0} kHz IQ",
                                    tuned / 1e6,
                                    sample_rate / 1000.0
                                ),
                            );
                        }
                    }
                    2 => {
                        last_iq = Instant::now();
                        protocol::samples(data, &mut iq)?;
                        let decoder = fm.as_mut().ok_or("IQ received before sample rate")?;
                        decoder.process(&iq, &mut pcm);
                        let gain = f32::from_bits(volume.load(Ordering::Relaxed));
                        let muted = Instant::now() < mute_until;
                        for x in &mut pcm {
                            *x *= if muted { 0.0 } else { gain };
                        }
                        // Interleaved left/right pairs; the stat counts frames.
                        total_audio += pcm.len() as u64 / 2;
                        if let Some(w) = wav.as_mut() {
                            w.write(&pcm).map_err(|e| e.to_string())?;
                        }
                        if let Some(a) = &output {
                            for chunk in pcm.chunks(1920) {
                                if !a.push(chunk) {
                                    dropped += 1;
                                }
                            }
                        }
                        // A station that gains or loses its pilot re-announces
                        // itself, so the panel never claims the wrong mode.
                        let now_stereo = decoder.stereo();
                        if !muted && !pcm.is_empty() && (!playing || now_stereo != stereo) {
                            stereo = now_stereo;
                            let mode = if stereo { "stereo" } else { "mono" };
                            println!(
                                "{}",
                                json!({
                                    "state": "playing",
                                    "message": format!("FM {:.1} MHz · {mode}", tuned / 1e6),
                                    "stereo": stereo,
                                })
                            );
                            playing = true;
                        }
                    }
                    3 => {
                        return Err(
                            "Server sent compressed IQ despite compression being disabled".into(),
                        );
                    }
                    6 => return Err(format!("SDR++ server reported a protocol error: {data:?}")),
                    _ => return Err(format!("Unsupported SDR++ packet type: {kind}")),
                }
            }
            if last_frame.elapsed() > Duration::from_secs(5)
                || last_iq.elapsed() > Duration::from_secs(8)
            {
                return Err(
                    "Timed out waiting for SDR++ samples; check the server's selected radio source"
                        .into(),
                );
            }
            if report.elapsed() >= Duration::from_secs(1) {
                println!(
                    "{}",
                    json!({"state":"stats", "sampleRate":sample_rate, "audioSamples":total_audio, "droppedAudioBlocks":dropped})
                );
                report = Instant::now();
            }
        }
        if opt.seconds.is_some() && total_audio == 0 {
            return Err("Test finished without receiving audio samples".into());
        }
        Ok(())
    })();
    if fm.is_some() {
        let _ = protocol::command(&mut socket, 3, &[]);
    }
    let _ = socket.shutdown(Shutdown::Both);
    if let Some(w) = wav.as_mut() {
        w.finish().map_err(|e| e.to_string())?;
    }
    result?;
    event("stopped", "Disconnected");
    Ok(())
}

fn main() {
    if let Err(e) = options().and_then(run) {
        event("error", &e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_comma_and_rejects_invalid_tuning() {
        assert_eq!(frequency("102,4").unwrap(), 102_400_000.0);
        assert!(frequency("NaN").is_err());
        assert!(frequency("108.1").is_err());
    }
}
