//! Just enough of a zip (jar) reader for Blink: find one file and read it. Paper's jar tells us what
//! to put on the class path, and the jar inside it knows its Minecraft version and protocol.

use std::io::Read;

/// Read one file from a zip archive in memory.
pub fn read(zip: &[u8], name: &str) -> Option<Vec<u8>> {
    let u16_at = |i: usize| zip.get(i..i + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize);
    let u32_at = |i: usize| zip.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);

    // End of central directory: the last "PK\x05\x06"
    let end = (0..zip.len().saturating_sub(21)).rev().find(|&i| zip[i..].starts_with(b"PK\x05\x06"))?;
    let entries = u16_at(end + 10)?;
    let mut at = u32_at(end + 16)?;

    for _ in 0..entries {
        if !zip.get(at..)?.starts_with(b"PK\x01\x02") {
            return None;
        }
        let method = u16_at(at + 10)?;
        let compressed = u32_at(at + 20)?;
        let size = u32_at(at + 24)?;
        let name_len = u16_at(at + 28)?;
        let extra_len = u16_at(at + 30)?;
        let comment_len = u16_at(at + 32)?;
        let local = u32_at(at + 42)?;
        if zip.get(at + 46..at + 46 + name_len)? == name.as_bytes() {
            let data_at = local + 30 + u16_at(local + 26)? + u16_at(local + 28)?;
            let data = zip.get(data_at..data_at + compressed)?;
            return match method {
                0 => Some(data.to_vec()),
                8 => {
                    let mut out = Vec::with_capacity(size);
                    flate2::read::DeflateDecoder::new(data).read_to_end(&mut out).ok()?;
                    Some(out)
                }
                _ => None,
            };
        }
        at += 46 + name_len + extra_len + comment_len;
    }
    None
}

/// Read one file from a zip file on disk.
pub fn read_file(path: &std::path::Path, name: &str) -> Option<Vec<u8>> {
    read(&std::fs::read(path).ok()?, name)
}
