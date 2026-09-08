//! SDR++ native server protocol, little endian (the supported Linux hosts).
//! Implemented from upstream server_protocol.h and sample_stream_decompressor.h.
use std::io::{self, Write};

pub const MAX_PACKET: usize = 8 * 1024 * 1024;

pub fn command(writer: &mut impl Write, command: u32, data: &[u8]) -> io::Result<()> {
    let mut packet = Vec::with_capacity(12 + data.len());
    packet.extend_from_slice(&0u32.to_le_bytes());
    packet.extend_from_slice(&((12 + data.len()) as u32).to_le_bytes());
    packet.extend_from_slice(&command.to_le_bytes());
    packet.extend_from_slice(data);
    writer.write_all(&packet)
}

#[derive(Default)]
pub struct Framer {
    bytes: Vec<u8>,
    start: usize,
}

impl Framer {
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.start != 0 {
            self.bytes.drain(..self.start);
            self.start = 0;
        }
        if self.bytes.len() + bytes.len() > MAX_PACKET + 65536 {
            return Err("Server exceeded the receive buffer limit".into());
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    pub fn next(&mut self) -> Result<Option<(u32, &[u8])>, String> {
        let b = &self.bytes[self.start..];
        if b.len() < 8 {
            return Ok(None);
        }
        let kind = u32::from_le_bytes(b[..4].try_into().unwrap());
        let size = u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize;
        if !(8..=MAX_PACKET).contains(&size) {
            return Err(format!("Invalid SDR++ packet size: {size}"));
        }
        if b.len() < size {
            return Ok(None);
        }
        self.start += size;
        Ok(Some((kind, &b[8..size])))
    }
}

pub fn samples(payload: &[u8], out: &mut Vec<[f32; 2]>) -> Result<(), String> {
    if payload.len() < 8 {
        return Err("Truncated IQ header".into());
    }
    let kind = u16::from_le_bytes(payload[2..4].try_into().unwrap());
    let scale = f32::from_le_bytes(payload[4..8].try_into().unwrap());
    let stride = match kind {
        0 => 2,
        1 => 4,
        2 => 8,
        _ => return Err("Unknown IQ sample format".into()),
    };
    if !(payload.len() - 8).is_multiple_of(stride) || !scale.is_finite() {
        return Err("Invalid IQ sample payload".into());
    }
    out.clear();
    for b in payload[8..].chunks_exact(stride) {
        let sample = match kind {
            0 => [
                b[0] as i8 as f32 * scale / 128.0,
                b[1] as i8 as f32 * scale / 128.0,
            ],
            1 => [
                i16::from_le_bytes(b[..2].try_into().unwrap()) as f32 * scale / 32768.0,
                i16::from_le_bytes(b[2..4].try_into().unwrap()) as f32 * scale / 32768.0,
            ],
            _ => [
                f32::from_le_bytes(b[..4].try_into().unwrap()),
                f32::from_le_bytes(b[4..8].try_into().unwrap()),
            ],
        };
        if !sample[0].is_finite() || !sample[1].is_finite() {
            return Err("Non-finite IQ sample".into());
        }
        out.push(sample);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fragmented_and_coalesced_frames() {
        let mut wire = vec![];
        command(&mut wire, 4, &102_400_000f64.to_le_bytes()).unwrap();
        command(&mut wire, 2, &[]).unwrap();
        let mut f = Framer::default();
        for b in &wire[..19] {
            f.push(&[*b]).unwrap();
            assert!(f.next().unwrap().is_none());
        }
        f.push(&wire[19..]).unwrap();
        assert_eq!(f.next().unwrap().unwrap().1[0], 4);
        assert_eq!(f.next().unwrap().unwrap().1, 2u32.to_le_bytes());
        assert!(f.next().unwrap().is_none());
    }
    #[test]
    fn rejects_invalid_lengths_and_samples() {
        let mut f = Framer::default();
        f.push(&[0; 8]).unwrap();
        assert!(f.next().is_err());
        let mut p = vec![0, 0, 1, 0];
        p.extend_from_slice(&2f32.to_le_bytes());
        p.extend_from_slice(&16384i16.to_le_bytes());
        p.extend_from_slice(&(-8192i16).to_le_bytes());
        let mut out = vec![];
        samples(&p, &mut out).unwrap();
        assert_eq!(out, [[1.0, -0.5]]);
        p.push(0);
        assert!(samples(&p, &mut out).is_err());
    }
}
