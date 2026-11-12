//! How a measurement is written next to its graph and how tall the graph
//! draws it.

use crate::resource::Resource;

/// How many layers the graph stacks: a value of 1.0 fills the graph once,
/// and anything above draws over it in a stronger shade, up to this many
/// times. For CPU, one layer is one core.
pub const LAYERS: f32 = 4.0;

/// How the memory graph turns bytes into height. The other resources are
/// always linear.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Scale {
    /// One layer per 512 MB.
    #[default]
    Linear,
    /// One layer each time the value grows fourfold: 128 MB fills one
    /// layer, 512 MB two, 2 GB three and 8 GB all four.
    Log,
}

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
/// - Memory: by `scale`, the same for every process, so the rows can be
///   compared with each other.
/// - Network: one layer per megabyte a second.
pub fn height(resource: Resource, value: Option<f64>, scale: Scale) -> f32 {
    let Some(v) = value else {
        return 0.0;
    };
    let layers = match resource {
        Resource::Cpu => v,
        Resource::Memory => match scale {
            Scale::Linear => v / (512.0 * MIB),
            // log4(v / 32 MB); 32 MB and below draw nothing.
            Scale::Log if v > 0.0 => (v / (32.0 * MIB)).log2() / 2.0,
            Scale::Log => 0.0,
        },
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
        let lin = Scale::Linear;
        assert_eq!(height(Cpu, Some(0.5), lin), 0.5);
        assert_eq!(height(Cpu, Some(9.0), lin), LAYERS);
        assert_eq!(height(Cpu, Some(-0.1), lin), 0.0);
        assert_eq!(height(Cpu, None, lin), 0.0);
        assert_eq!(height(Memory, Some(256.0 * MIB), lin), 0.5);
        assert_eq!(height(Memory, Some(1.5 * GIB), lin), 3.0);
        assert_eq!(height(Memory, Some(8.0 * GIB), lin), LAYERS);
        assert_eq!(height(NetworkIn, Some(2e6), lin), 2.0);
    }

    #[test]
    fn log_memory_gains_a_layer_each_fourfold() {
        let log = Scale::Log;
        assert_eq!(height(Memory, Some(32.0 * MIB), log), 0.0);
        assert_eq!(height(Memory, Some(64.0 * MIB), log), 0.5);
        assert_eq!(height(Memory, Some(128.0 * MIB), log), 1.0);
        assert_eq!(height(Memory, Some(512.0 * MIB), log), 2.0);
        assert_eq!(height(Memory, Some(2.0 * GIB), log), 3.0);
        assert_eq!(height(Memory, Some(8.0 * GIB), log), LAYERS);
        assert_eq!(height(Memory, Some(64.0 * GIB), log), LAYERS);
        assert_eq!(height(Memory, Some(1.0), log), 0.0);
        assert_eq!(height(Memory, Some(0.0), log), 0.0);
    }

    #[test]
    fn only_memory_follows_the_scale() {
        assert_eq!(height(Cpu, Some(2.0), Scale::Log), 2.0);
        assert_eq!(height(NetworkOut, Some(1e6), Scale::Log), 1.0);
    }
}
