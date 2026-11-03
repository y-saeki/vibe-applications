//! What the macOS app calls, through the Swift bindings UniFFI writes from
//! this file (macos/build.sh). Built with the `ffi` feature.

use std::sync::Arc;

use crate::art;
use crate::monitor::Row;
use crate::resource::Resource;
use crate::runner::Runner;

/// Told after every reading, on the reading thread.
#[uniffi::export(with_foreign)]
pub trait Listener: Send + Sync {
    fn updated(&self);
}

/// The process history, read once a second from the moment it is made.
#[derive(uniffi::Object)]
pub struct Vitals {
    runner: Runner,
}

#[uniffi::export]
impl Vitals {
    #[uniffi::constructor]
    pub fn new(network: bool, listener: Arc<dyn Listener>) -> Arc<Self> {
        Arc::new(Self {
            runner: Runner::start(read, network, Box::new(move || listener.updated())),
        })
    }

    pub fn set_network(&self, on: bool) {
        self.runner.set_network(on);
    }

    /// The `max` processes that used the most of `resource` in the last
    /// minute.
    pub fn top(&self, resource: Resource, max: u32) -> Vec<Row> {
        self.runner.top(resource, max as usize)
    }
}

#[cfg(target_os = "macos")]
fn read(network: bool) -> Option<crate::sample::Samples> {
    crate::mac_sample::read(network)
}

/// Elsewhere there is nothing to read; this keeps `cargo test --features
/// ffi` building on every platform.
#[cfg(not(target_os = "macos"))]
fn read(_network: bool) -> Option<crate::sample::Samples> {
    None
}

#[uniffi::export]
pub fn resource_title(resource: Resource) -> String {
    resource.title().to_string()
}

/// The resource the arrows move to: `step` is -1 for the left one and 1 for
/// the right.
#[uniffi::export]
pub fn cycle_resource(resource: Resource, step: i32, network: bool) -> Resource {
    resource.cycle(step, network)
}

/// `resource`, or CPU when it is a network one and `network` is off.
#[uniffi::export]
pub fn available_resource(resource: Resource, network: bool) -> Resource {
    resource.or_available(network)
}

/// How many layers a graph stacks; see `format::LAYERS`.
#[uniffi::export]
pub fn chart_layers() -> f32 {
    crate::format::LAYERS
}

/// The app icon, `size` pixels square, as R, G, B, A bytes.
#[uniffi::export]
pub fn app_icon_rgba(size: u32) -> Vec<u8> {
    art::rgba_bytes(&art::mac_pixels(size as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arrows_skip_network_while_it_is_off() {
        assert_eq!(cycle_resource(Resource::Memory, 1, false), Resource::Cpu);
        assert_eq!(
            cycle_resource(Resource::Memory, 1, true),
            Resource::NetworkIn
        );
        assert_eq!(
            available_resource(Resource::NetworkOut, false),
            Resource::Cpu
        );
    }

    #[test]
    fn the_app_icon_is_rgba() {
        assert_eq!(app_icon_rgba(16).len(), 16 * 16 * 4);
    }
}
