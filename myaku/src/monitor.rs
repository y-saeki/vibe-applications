//! The last minute of every process, kept from one reading a second, and the
//! list made from it.

use std::collections::HashMap;

use crate::format;
use crate::resource::Resource;
use crate::sample::{Sample, Samples};
use crate::stats::StatWindow;

/// One line of the list.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Row {
    pub pid: u32,
    pub name: String,
    /// False once the process has exited; it stays in the list, drawn
    /// faded, until its last minute has scrolled out of the graph.
    pub alive: bool,
    /// The graph, oldest first, one value a second, each in layers
    /// (see [`format::LAYERS`]).
    pub chart: Vec<f32>,
    /// The newest value, written out.
    pub value: String,
}

#[derive(Debug)]
struct Process {
    name: String,
    alive: bool,
    last: Sample,
    cpu: StatWindow,
    memory: StatWindow,
    net_in: StatWindow,
    net_out: StatWindow,
}

impl Process {
    fn new(sample: Sample) -> Self {
        Self {
            name: sample.name.clone(),
            alive: true,
            last: sample,
            cpu: StatWindow::default(),
            memory: StatWindow::default(),
            net_in: StatWindow::default(),
            net_out: StatWindow::default(),
        }
    }

    /// Whether `sample` is a different process that was given the same ID
    /// after this one exited.
    fn is_replaced_by(&self, sample: &Sample) -> bool {
        sample.name != self.last.name || sample.cpu_time < self.last.cpu_time
    }

    fn add(&mut self, sample: Option<Sample>, elapsed: f64) {
        let Some(sample) = sample else {
            self.alive = false;
            for w in self.windows_mut() {
                w.push(None);
            }
            return;
        };
        let cpu =
            (elapsed > 0.0).then(|| (sample.cpu_time - self.last.cpu_time).max(0.0) / elapsed);
        self.cpu.push(cpu);
        self.memory.push(Some(sample.memory as f64));
        // The counters can go down a little between readings (nettop does,
        // without saying why), so a decrease counts as nothing.
        let delta = |now: Option<u64>, before: Option<u64>| match (now, before) {
            (Some(now), Some(before)) => Some(now.saturating_sub(before) as f64),
            _ => None,
        };
        self.net_in.push(delta(sample.net_in, self.last.net_in));
        self.net_out.push(delta(sample.net_out, self.last.net_out));
        self.alive = true;
        self.last = sample;
    }

    fn windows_mut(&mut self) -> [&mut StatWindow; 4] {
        [
            &mut self.cpu,
            &mut self.memory,
            &mut self.net_in,
            &mut self.net_out,
        ]
    }

    fn window(&self, resource: Resource) -> &StatWindow {
        match resource {
            Resource::Cpu => &self.cpu,
            Resource::Memory => &self.memory,
            Resource::NetworkIn => &self.net_in,
            Resource::NetworkOut => &self.net_out,
        }
    }

    /// An exited process is forgotten once nothing of it is left to draw.
    fn is_stale(&self) -> bool {
        !self.alive && self.cpu.is_empty() && self.memory.is_empty()
    }
}

#[derive(Debug, Default)]
pub struct Monitor {
    processes: HashMap<u32, Process>,
}

impl Monitor {
    /// Takes a reading of every running process, `elapsed` seconds after
    /// the previous one.
    pub fn add(&mut self, mut samples: Samples, elapsed: f64) {
        self.processes.retain(|pid, process| {
            let sample = samples.remove_entry(pid);
            match sample {
                Some((_, sample)) if process.is_replaced_by(&sample) => {
                    // Put back so that it starts afresh below.
                    samples.insert(*pid, sample);
                    false
                }
                Some((_, sample)) => {
                    process.add(Some(sample), elapsed);
                    true
                }
                None => {
                    process.add(None, elapsed);
                    !process.is_stale()
                }
            }
        });
        // The ones not seen before: measured from the next reading, as their
        // CPU use needs two.
        for (pid, sample) in samples {
            self.processes.insert(pid, Process::new(sample));
        }
    }

    /// The `max` processes that used the most of `resource` over the last
    /// minute, most first, with memory graphs drawn by `scale`.
    pub fn top(&self, resource: Resource, max: usize, scale: format::Scale) -> Vec<Row> {
        let mut ranked: Vec<(&u32, &Process)> = self.processes.iter().collect();
        ranked.sort_by(|(pa, a), (pb, b)| {
            let (sa, sb) = (a.window(resource).sum(), b.window(resource).sum());
            sb.total_cmp(&sa).then(pa.cmp(pb))
        });
        ranked
            .into_iter()
            .take(max)
            .map(|(&pid, p)| {
                let w = p.window(resource);
                Row {
                    pid,
                    name: p.name.clone(),
                    alive: p.alive,
                    chart: w
                        .values()
                        .map(|v| format::height(resource, v, scale))
                        .collect(),
                    value: format::value(resource, w.latest()),
                }
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.processes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.processes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::Scale;
    use crate::stats::WINDOW;

    fn reading(list: &[(u32, &str, f64, u64)]) -> Samples {
        list.iter()
            .map(|&(pid, name, cpu, mem)| (pid, Sample::new(name, cpu, mem)))
            .collect()
    }

    #[test]
    fn cpu_is_the_cpu_time_used_per_second_between_readings() {
        let mut m = Monitor::default();
        m.add(reading(&[(1, "a", 10.0, 0)]), 1.0);
        // A single reading measures nothing yet.
        assert_eq!(m.top(Resource::Cpu, 5, Scale::Linear)[0].value, "-");
        m.add(reading(&[(1, "a", 11.0, 0)]), 2.0);
        let row = &m.top(Resource::Cpu, 5, Scale::Linear)[0];
        assert_eq!(row.value, "50%");
        assert_eq!(row.chart.len(), WINDOW);
        assert_eq!(row.chart[WINDOW - 1], 0.5);
        assert!(row.chart[..WINDOW - 1].iter().all(|&v| v == 0.0));
    }

    #[test]
    fn ranks_by_the_whole_minute_not_the_last_second() {
        let mut m = Monitor::default();
        m.add(
            reading(&[(1, "busy-before", 0.0, 0), (2, "busy-now", 0.0, 0)]),
            1.0,
        );
        m.add(
            reading(&[(1, "busy-before", 5.0, 0), (2, "busy-now", 0.0, 0)]),
            1.0,
        );
        m.add(
            reading(&[(1, "busy-before", 5.0, 0), (2, "busy-now", 1.0, 0)]),
            1.0,
        );
        let names: Vec<_> = m
            .top(Resource::Cpu, 5, Scale::Linear)
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert_eq!(names, ["busy-before", "busy-now"]);
    }

    #[test]
    fn keeps_only_the_top_ones() {
        let mut m = Monitor::default();
        let list: Vec<_> = (1..=30).map(|i| (i, "p", 0.0, i as u64 * 100)).collect();
        m.add(reading(&list), 1.0);
        m.add(reading(&list), 1.0);
        let top = m.top(Resource::Memory, 20, Scale::Linear);
        assert_eq!(top.len(), 20);
        assert_eq!(top[0].pid, 30);
        assert_eq!(top[19].pid, 11);
    }

    #[test]
    fn memory_graph_does_not_shrink_as_processes_are_added() {
        const MIB: u64 = 1024 * 1024;
        let mut m = Monitor::default();
        let mut list = vec![(1, "a", 0.0, 512 * MIB)];
        list.extend((2..=300).map(|pid| (pid, "other", 0.0, 100 * MIB)));
        m.add(reading(&list), 1.0);
        m.add(reading(&list), 1.0);
        let top = m.top(Resource::Memory, 5, Scale::Linear);
        assert_eq!(top[0].chart[WINDOW - 1], 1.0);
        assert!((top[1].chart[WINDOW - 1] - 100.0 / 512.0).abs() < 1e-6);
    }

    #[test]
    fn memory_graph_follows_the_scale() {
        const MIB: u64 = 1024 * 1024;
        let mut m = Monitor::default();
        let list = [(1, "a", 0.0, 2048 * MIB)];
        m.add(reading(&list), 1.0);
        m.add(reading(&list), 1.0);
        assert_eq!(
            m.top(Resource::Memory, 1, Scale::Linear)[0].chart[WINDOW - 1],
            4.0
        );
        let log = m.top(Resource::Memory, 1, Scale::Log)[0].chart[WINDOW - 1];
        assert!((log - 0.7).abs() < 1e-6);
    }

    #[test]
    fn an_exited_process_stays_faded_for_a_minute() {
        let mut m = Monitor::default();
        m.add(reading(&[(1, "a", 0.0, 10)]), 1.0);
        m.add(reading(&[(1, "a", 1.0, 10)]), 1.0);
        m.add(reading(&[]), 1.0);
        let row = &m.top(Resource::Cpu, 5, Scale::Linear)[0];
        assert!(!row.alive);
        assert_eq!(row.value, "-");
        for _ in 0..WINDOW - 2 {
            m.add(reading(&[]), 1.0);
        }
        assert_eq!(m.len(), 1);
        m.add(reading(&[]), 1.0);
        assert!(m.is_empty());
    }

    #[test]
    fn a_process_that_comes_back_is_alive_again() {
        let mut m = Monitor::default();
        m.add(reading(&[(1, "a", 0.0, 10)]), 1.0);
        m.add(reading(&[]), 1.0);
        m.add(reading(&[(1, "a", 0.0, 10)]), 1.0);
        assert!(m.top(Resource::Cpu, 5, Scale::Linear)[0].alive);
    }

    #[test]
    fn a_reused_process_id_starts_afresh() {
        let mut m = Monitor::default();
        m.add(reading(&[(1, "old", 0.0, 10)]), 1.0);
        m.add(reading(&[(1, "old", 50.0, 10)]), 1.0);
        m.add(reading(&[(1, "new", 0.2, 10)]), 1.0);
        let row = &m.top(Resource::Cpu, 5, Scale::Linear)[0];
        assert_eq!(row.name, "new");
        assert_eq!(row.value, "-");
        assert!(row.chart.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn network_is_bytes_per_reading_and_never_negative() {
        let mut m = Monitor::default();
        let with_net = |net_in: u64, net_out: u64| {
            let mut s = Sample::new("a", 0.0, 0);
            s.net_in = Some(net_in);
            s.net_out = Some(net_out);
            Samples::from([(1, s)])
        };
        m.add(with_net(1000, 1000), 1.0);
        m.add(with_net(1000 + 2048, 900), 1.0);
        assert_eq!(
            m.top(Resource::NetworkIn, 1, Scale::Linear)[0].value,
            "2 KB"
        );
        assert_eq!(
            m.top(Resource::NetworkOut, 1, Scale::Linear)[0].value,
            "0 KB"
        );
        // Without network statistics the network has nothing to show.
        m.add(reading(&[(1, "a", 0.0, 0)]), 1.0);
        assert_eq!(m.top(Resource::NetworkIn, 1, Scale::Linear)[0].value, "-");
    }
}
