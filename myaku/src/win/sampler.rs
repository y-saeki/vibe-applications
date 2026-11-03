//! Reading every process with one `NtQuerySystemInformation` call. The list
//! it fills in is parsed by `myaku::win_sample`.

use std::cell::RefCell;

use windows::Wdk::System::SystemInformation::{NtQuerySystemInformation, SystemProcessInformation};
use windows::Win32::Foundation::STATUS_INFO_LENGTH_MISMATCH;

use myaku::sample::Samples;

thread_local! {
    /// Kept between readings: the list is a few hundred kilobytes, and
    /// allocating it once a second is waste. `u64` keeps it 8-byte aligned,
    /// as the entries are.
    static BUFFER: RefCell<Vec<u64>> = RefCell::new(vec![0; 64 * 1024]);
}

/// Every process's CPU time and memory. Windows offers no per-process
/// network statistics to an ordinary user, so `network` changes nothing.
pub fn read(_network: bool) -> Option<Samples> {
    BUFFER.with_borrow_mut(|buffer| {
        for _ in 0..8 {
            let bytes = (buffer.len() * 8) as u32;
            let mut needed = 0u32;
            let status = unsafe {
                NtQuerySystemInformation(
                    SystemProcessInformation,
                    buffer.as_mut_ptr().cast(),
                    bytes,
                    &mut needed,
                )
            };
            if status == STATUS_INFO_LENGTH_MISMATCH {
                // Processes can start between asking and reading, so leave
                // room for more.
                let words = (needed as usize).max(bytes as usize) / 8 * 3 / 2;
                buffer.resize(words, 0);
                continue;
            }
            if status.is_err() {
                return None;
            }
            let used = (needed as usize).min(buffer.len() * 8);
            // SAFETY: the buffer is `buffer.len() * 8` bytes, of which the
            // call wrote `used`.
            let view = unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<u8>(), used) };
            return Some(myaku::win_sample::parse(view, buffer.as_ptr() as usize));
        }
        None
    })
}
