//! The background thread that reads every process once a second, from the
//! moment myaku starts, so that the last minute is there whenever the list
//! is opened.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::format::Scale;
use crate::monitor::{Monitor, Row};
use crate::resource::Resource;
use crate::sample::Samples;

pub const INTERVAL: Duration = Duration::from_secs(1);

/// Reads every process; with `true`, network statistics too. `None` when
/// the operating system could not be asked this time.
pub type Reader = fn(network: bool) -> Option<Samples>;

struct Shared {
    monitor: Mutex<Monitor>,
    network: AtomicBool,
    stop: AtomicBool,
}

pub struct Runner {
    shared: Arc<Shared>,
}

impl Runner {
    /// Starts reading with `read`, calling `updated` on the reading thread
    /// after each reading has been added.
    pub fn start(read: Reader, network: bool, updated: Box<dyn Fn() + Send>) -> Self {
        let shared = Arc::new(Shared {
            monitor: Mutex::new(Monitor::default()),
            network: AtomicBool::new(network),
            stop: AtomicBool::new(false),
        });
        let worker = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("myaku-sampler".into())
            .spawn(move || run(&worker, read, updated))
            .expect("starting the sampling thread");
        Self { shared }
    }

    pub fn set_network(&self, on: bool) {
        self.shared.network.store(on, Ordering::Relaxed);
    }

    pub fn top(&self, resource: Resource, max: usize, scale: Scale) -> Vec<Row> {
        lock(&self.shared.monitor).top(resource, max, scale)
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}

fn lock(m: &Mutex<Monitor>) -> std::sync::MutexGuard<'_, Monitor> {
    // A panic elsewhere cannot leave the history half-written in a way that
    // matters more than showing it.
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn run(shared: &Shared, read: Reader, updated: Box<dyn Fn() + Send>) {
    let mut previous: Option<Instant> = None;
    let mut next = Instant::now();
    while !shared.stop.load(Ordering::Relaxed) {
        if let Some(samples) = read(shared.network.load(Ordering::Relaxed)) {
            let now = Instant::now();
            let elapsed = previous.map_or(0.0, |p| (now - p).as_secs_f64());
            previous = Some(now);
            lock(&shared.monitor).add(samples, elapsed);
            updated();
        }
        next += INTERVAL;
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            // Fell behind (the machine slept, or a reading took too long):
            // start counting again instead of catching up in a burst.
            next = now;
        }
    }
}
