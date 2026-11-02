//! One reading of every running process, as the operating system reports it.

use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    /// What the list calls the process.
    pub name: String,
    /// CPU time used since the process started, in seconds.
    pub cpu_time: f64,
    /// Memory in use, in bytes.
    pub memory: u64,
    /// Bytes received and sent since some point before, when network
    /// statistics are taken. Only the difference between readings counts.
    pub net_in: Option<u64>,
    pub net_out: Option<u64>,
}

impl Sample {
    pub fn new(name: impl Into<String>, cpu_time: f64, memory: u64) -> Self {
        Self {
            name: name.into(),
            cpu_time,
            memory,
            net_in: None,
            net_out: None,
        }
    }
}

/// A reading of every process, by process ID.
pub type Samples = HashMap<u32, Sample>;
