//! What the list ranks the processes by.

use crate::format::Scale;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Resource {
    Cpu,
    Memory,
    NetworkIn,
    NetworkOut,
}

impl Resource {
    pub const ALL: [Resource; 4] = [
        Resource::Cpu,
        Resource::Memory,
        Resource::NetworkIn,
        Resource::NetworkOut,
    ];

    /// The list's heading.
    pub fn title(self) -> &'static str {
        match self {
            Resource::Cpu => "CPU",
            Resource::Memory => "メモリ",
            Resource::NetworkIn => "ネットワーク(受信)",
            Resource::NetworkOut => "ネットワーク(送信)",
        }
    }

    /// The list's heading when memory graphs are drawn by `scale`: the
    /// title, marked when the graph is logarithmic.
    pub fn heading(self, scale: Scale) -> &'static str {
        match (self, scale) {
            (Resource::Memory, Scale::Log) => "メモリ(対数)",
            _ => self.title(),
        }
    }

    pub fn is_network(self) -> bool {
        matches!(self, Resource::NetworkIn | Resource::NetworkOut)
    }

    /// The resources the list can show: the network ones only while network
    /// statistics are being taken.
    pub fn available(network: bool) -> Vec<Resource> {
        Self::ALL
            .into_iter()
            .filter(|r| network || !r.is_network())
            .collect()
    }

    /// The resource `step` places after this one among the available ones,
    /// wrapping around at either end. One that is not available (network,
    /// just switched off) counts as CPU.
    pub fn cycle(self, step: i32, network: bool) -> Resource {
        let list = Self::available(network);
        let here = list.iter().position(|&r| r == self).unwrap_or(0) as i32;
        let n = list.len() as i32;
        list[(here + step).rem_euclid(n) as usize]
    }

    /// This resource if it is available, CPU if not.
    pub fn or_available(self, network: bool) -> Resource {
        if network || !self.is_network() {
            self
        } else {
            Resource::Cpu
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Resource::*;

    #[test]
    fn cycles_through_all_four_with_network() {
        assert_eq!(Cpu.cycle(1, true), Memory);
        assert_eq!(Memory.cycle(1, true), NetworkIn);
        assert_eq!(NetworkOut.cycle(1, true), Cpu);
        assert_eq!(Cpu.cycle(-1, true), NetworkOut);
    }

    #[test]
    fn cycles_between_cpu_and_memory_without_network() {
        assert_eq!(Cpu.cycle(1, false), Memory);
        assert_eq!(Memory.cycle(1, false), Cpu);
        assert_eq!(Cpu.cycle(-1, false), Memory);
        assert_eq!(Memory.cycle(-1, false), Cpu);
    }

    #[test]
    fn a_network_resource_falls_back_to_cpu_once_switched_off() {
        assert_eq!(NetworkIn.or_available(false), Cpu);
        assert_eq!(NetworkOut.or_available(true), NetworkOut);
        assert_eq!(Memory.or_available(false), Memory);
        assert_eq!(NetworkOut.cycle(1, false), Memory);
    }

    #[test]
    fn the_heading_marks_a_logarithmic_memory_graph() {
        assert_eq!(Memory.heading(Scale::Linear), "メモリ");
        assert_eq!(Memory.heading(Scale::Log), "メモリ(対数)");
        assert_eq!(Cpu.heading(Scale::Log), "CPU");
    }
}
