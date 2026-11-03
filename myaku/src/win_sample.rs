//! Reading the processes on Windows, from the list
//! `NtQuerySystemInformation(SystemProcessInformation)` fills in.
//!
//! One call returns every process, other users' and services' included,
//! with its CPU time and memory, which is why it is used rather than opening
//! each process. The list's layout is read here by offset, so that it is
//! tested on every platform; `win/sampler.rs` makes the call.

use crate::sample::{Sample, Samples};

/// Offsets into a 64-bit `SYSTEM_PROCESS_INFORMATION`.
mod offset {
    pub const NEXT_ENTRY: usize = 0;
    pub const WORKING_SET_PRIVATE: usize = 8;
    pub const USER_TIME: usize = 40;
    pub const KERNEL_TIME: usize = 48;
    pub const IMAGE_NAME_LENGTH: usize = 56;
    pub const IMAGE_NAME_BUFFER: usize = 64;
    pub const PROCESS_ID: usize = 80;
    /// Enough to hold every field above.
    pub const MIN_ENTRY: usize = 88;
}

/// The system's own idle time, which is not a process anyone can do
/// anything about.
const IDLE_PID: u32 = 0;

/// Parses the list in `buffer`, which the call filled in at address `base`:
/// the names are pointers into the same buffer.
pub fn parse(buffer: &[u8], base: usize) -> Samples {
    let mut samples = Samples::new();
    let mut at = 0usize;
    while let Some(entry) = buffer.get(at..).filter(|e| e.len() >= offset::MIN_ENTRY) {
        let next = u32_at(entry, offset::NEXT_ENTRY) as usize;
        let pid = u64_at(entry, offset::PROCESS_ID) as u32;
        if pid != IDLE_PID {
            let cpu = (u64_at(entry, offset::USER_TIME) + u64_at(entry, offset::KERNEL_TIME))
                as f64
                / 1e7;
            let memory = u64_at(entry, offset::WORKING_SET_PRIVATE);
            let name = image_name(buffer, base, entry).unwrap_or_else(|| match pid {
                4 => "System".to_string(),
                _ => format!("PID {pid}"),
            });
            samples.insert(pid, Sample::new(name, cpu, memory));
        }
        if next == 0 {
            break;
        }
        at += next;
    }
    samples
}

/// The image name without `.exe`, or `None` when it is empty or does not lie
/// inside the buffer.
fn image_name(buffer: &[u8], base: usize, entry: &[u8]) -> Option<String> {
    let len = u16::from_le_bytes([
        entry[offset::IMAGE_NAME_LENGTH],
        entry[offset::IMAGE_NAME_LENGTH + 1],
    ]) as usize;
    let ptr = u64_at(entry, offset::IMAGE_NAME_BUFFER) as usize;
    if len == 0 || ptr < base {
        return None;
    }
    let start = ptr - base;
    let bytes = buffer.get(start..start.checked_add(len)?)?;
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| u16::from_le_bytes(c))
        .collect();
    let name = String::from_utf16_lossy(&units);
    let trimmed = match name.len().checked_sub(4) {
        Some(cut) if name.is_char_boundary(cut) && name[cut..].eq_ignore_ascii_case(".exe") => {
            &name[..cut]
        }
        _ => &name,
    };
    Some(trimmed.to_string())
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

fn u64_at(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: usize = 0x10_0000;

    /// Builds a list as the call would, with each entry's name after it.
    fn list(entries: &[(u32, &str, u64, u64, u64)]) -> Vec<u8> {
        let mut buf = Vec::new();
        for (i, &(pid, name, user, kernel, private)) in entries.iter().enumerate() {
            let start = buf.len();
            let mut entry = vec![0u8; 256];
            let units: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
            let name_at = start + entry.len();
            entry[8..16].copy_from_slice(&private.to_le_bytes());
            entry[40..48].copy_from_slice(&user.to_le_bytes());
            entry[48..56].copy_from_slice(&kernel.to_le_bytes());
            entry[56..58].copy_from_slice(&(units.len() as u16).to_le_bytes());
            let ptr = if units.is_empty() { 0 } else { BASE + name_at };
            entry[64..72].copy_from_slice(&(ptr as u64).to_le_bytes());
            entry[80..88].copy_from_slice(&(pid as u64).to_le_bytes());
            entry.extend(&units);
            entry.resize(entry.len().next_multiple_of(8), 0);
            let next = if i + 1 == entries.len() {
                0
            } else {
                entry.len() as u32
            };
            entry[0..4].copy_from_slice(&next.to_le_bytes());
            buf.extend(entry);
        }
        buf
    }

    #[test]
    fn reads_every_process_but_idle() {
        let buf = list(&[
            (0, "", 99, 99, 0),
            (4, "", 0, 20_000_000, 4096),
            (1234, "chrome.exe", 10_000_000, 5_000_000, 1 << 20),
            (88, "Registry", 0, 0, 12),
            (77, "Setup.EXE", 0, 0, 0),
        ]);
        let s = parse(&buf, BASE);
        assert_eq!(s.len(), 4);
        assert_eq!(s[&4], Sample::new("System", 2.0, 4096));
        assert_eq!(s[&1234], Sample::new("chrome", 1.5, 1 << 20));
        assert_eq!(s[&88].name, "Registry");
        assert_eq!(s[&77].name, "Setup");
    }

    #[test]
    fn stops_at_a_truncated_list_or_a_stray_pointer() {
        let mut buf = list(&[(10, "a.exe", 0, 0, 0), (20, "b.exe", 0, 0, 0)]);
        // A name pointing outside the buffer falls back to the ID.
        buf[64..72].copy_from_slice(&(1u64 << 40).to_le_bytes());
        let s = parse(&buf[..buf.len() - 300], BASE);
        assert_eq!(s.len(), 1);
        assert_eq!(s[&10].name, "PID 10");
        assert!(parse(&[], BASE).is_empty());
    }
}
