//! End-to-end checks against a small in-process SDR++ server, including the
//! command pipe used by QML. No real radio or audio device is needed.
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn packet(kind: u32, data: &[u8]) -> Vec<u8> {
    let mut b = kind.to_le_bytes().to_vec();
    b.extend_from_slice(&((data.len() + 8) as u32).to_le_bytes());
    b.extend_from_slice(data);
    b
}
fn listener() -> TcpListener {
    TcpListener::bind("127.0.0.1:0").unwrap()
}
fn client(listener: &TcpListener) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_omasdr"))
        .args([
            "--server",
            &listener.local_addr().unwrap().to_string(),
            "--no-audio",
            "--seconds",
            "3",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap()
}
fn connection(l: &TcpListener) -> TcpStream {
    l.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        if let Ok((s, _)) = l.accept() {
            s.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
            s.set_write_timeout(Some(Duration::from_secs(4))).unwrap();
            return s;
        }
        assert!(Instant::now() < deadline, "Client did not connect");
        thread::sleep(Duration::from_millis(10));
    }
}
fn read_command(s: &mut TcpStream) -> (u32, Vec<u8>) {
    let mut h = [0; 8];
    s.read_exact(&mut h).unwrap();
    assert_eq!(&h[..4], &0u32.to_le_bytes());
    let n = u32::from_le_bytes(h[4..].try_into().unwrap()) as usize;
    assert!((12..1024).contains(&n));
    let mut b = vec![0; n - 8];
    s.read_exact(&mut b).unwrap();
    (
        u32::from_le_bytes(b[..4].try_into().unwrap()),
        b[4..].to_vec(),
    )
}

#[test]
fn busy_server_is_reported_without_commands() {
    let l = listener();
    let child = client(&l);
    let mut s = connection(&l);
    s.write_all(&packet(0, &0x81u32.to_le_bytes())).unwrap();
    drop(s);
    let out = child.wait_with_output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("server is busy"));
}

#[test]
fn malformed_packet_is_rejected() {
    let l = listener();
    let child = client(&l);
    let mut s = connection(&l);
    s.write_all(&[0, 0, 0, 0, 255, 255, 255, 255]).unwrap();
    drop(s);
    let out = child.wait_with_output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("Invalid SDR++ packet size"));
}

#[test]
fn starts_receives_retunes_and_disconnects_over_stdin() {
    let l = listener();
    let mut child = client(&l);
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let _ = tx.send(line.unwrap());
        }
    });
    let mut s = connection(&l);
    let mut rate = 0x80u32.to_le_bytes().to_vec();
    rate.extend_from_slice(&250_000f64.to_le_bytes());
    // Every split in the TCP header/payload must be harmless.
    for byte in packet(0, &rate) {
        s.write_all(&[byte]).unwrap();
    }
    assert_eq!(read_command(&mut s), (6, vec![1]));
    assert_eq!(read_command(&mut s), (7, vec![0]));
    assert_eq!(
        read_command(&mut s),
        (4, 102_400_000f64.to_le_bytes().to_vec())
    );
    assert_eq!(read_command(&mut s), (2, vec![]));
    let mut iq = vec![0, 0, 1, 0];
    iq.extend_from_slice(&1f32.to_le_bytes());
    for _ in 0..4096 {
        iq.extend_from_slice(&[0, 64, 0, 0]);
    }
    s.write_all(&packet(2, &iq)).unwrap();
    loop {
        if rx
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .contains("\"playing\"")
        {
            break;
        }
    }
    let stdin = child.stdin.as_mut().unwrap();
    writeln!(stdin, "{{\"frequency\":103.1,\"volume\":0}}").unwrap();
    assert_eq!(
        read_command(&mut s),
        (4, 103_100_000f64.to_le_bytes().to_vec())
    );
    writeln!(stdin, "{{\"stop\":true}}").unwrap();
    assert_eq!(read_command(&mut s), (3, vec![]));
    assert!(child.wait().unwrap().success());
}
