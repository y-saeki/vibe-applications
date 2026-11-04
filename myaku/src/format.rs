//! How a measurement is written next to its graph and how tall the graph
//! draws it.

use crate::resource::Resource;

/// How many layers the graph stacks: a value of 1.0 fills the graph once,
/// and anything above draws over it in a stronger shade, up to this many
/// times. For CPU, one layer is one core.
pub const LAYERS: f32 = 4.0;

const KIB: f64 = 1024.0;
const MIB: f64 = KIB * 1024.0;
const GIB: f64 = MIB * 1024.0;

/// The text beside the graph for the newest value, or a dash when there is
/// none (the process exited, or the network was not read).
///
/// - CPU: the share of one core, as a percentage (`250%` is two and a half
///   cores).
/// - Memory: bytes, in MB below a gigabyte and in GB with one decimal above.
/// - Network: bytes in the last second.
pub fn value(resource: Resource, value: Option<f64>) -> String {
    let Some(v) = value else {
        return "-".to_string();
    };
    match resource {
        Resource::Cpu => format!("{:.0}%", v * 100.0),
        Resource::Memory => bytes(v),
        Resource::NetworkIn | Resource::NetworkOut => bytes(v),
    }
}

fn bytes(v: f64) -> String {
    if v < MIB {
        format!("{:.0} KB", v / KIB)
    } else if v < GIB {
        format!("{:.0} MB", v / MIB)
    } else {
        format!("{:.1} GB", v / GIB)
    }
}

/// How tall the graph draws a value, in layers (see [`LAYERS`]), between 0
/// and `LAYERS`. A missing value draws nothing.
///
/// - CPU: one layer per core.
/// - Memory: one layer per quarter of the memory all processes use together,
///   so a process using a quarter of it fills the graph.
/// - Network: one layer per megabyte a second.
pub fn height(resource: Resource, value: Option<f64>, total_memory: f64) -> f32 {
    let Some(v) = value else {
        return 0.0;
    };
    let layers = match resource {
        Resource::Cpu => v,
        Resource::Memory if total_memory > 0.0 => v / (total_memory / 4.0),
        Resource::Memory => 0.0,
        Resource::NetworkIn | Resource::NetworkOut => v / 1e6,
    };
    (layers as f32).clamp(0.0, LAYERS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Resource::*;

    #[test]
    fn cpu_is_a_percentage_of_one_core() {
        assert_eq!(value(Cpu, Some(0.034)), "3%");
        assert_eq!(value(Cpu, Some(2.5)), "250%");
        assert_eq!(value(Cpu, None), "-");
    }

    #[test]
    fn bytes_are_in_kb_mb_or_gb() {
        assert_eq!(value(Memory, Some(512.0 * KIB)), "512 KB");
        assert_eq!(value(Memory, Some(300.0 * MIB)), "300 MB");
        assert_eq!(value(Memory, Some(1.5 * GIB)), "1.5 GB");
        assert_eq!(value(NetworkIn, Some(0.0)), "0 KB");
        assert_eq!(value(NetworkOut, Some(2.0 * MIB)), "2 MB");
        assert_eq!(value(Memory, None), "-");
    }

    #[test]
    fn heights_are_in_layers_and_capped() {
        assert_eq!(height(Cpu, Some(0.5), 0.0), 0.5);
        assert_eq!(height(Cpu, Some(9.0), 0.0), LAYERS);
        assert_eq!(height(Cpu, Some(-0.1), 0.0), 0.0);
        assert_eq!(height(Cpu, None, 0.0), 0.0);
        assert_eq!(height(Memory, Some(25.0), 100.0), 1.0);
        assert_eq!(height(Memory, Some(25.0), 0.0), 0.0);
        assert_eq!(height(NetworkIn, Some(2e6), 0.0), 2.0);
    }
}
