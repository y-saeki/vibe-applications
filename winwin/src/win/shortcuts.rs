//! A config's shortcuts registered on one window, and the presses followed
//! through `cycle::Presses`: the resident part's hidden window and the test
//! window each hold one.
//!
//! Registering, unregistering and the timers post nothing to the window, so
//! all of it may be called while its owner's state is borrowed.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey,
};
use windows::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer};

use super::modifiers_held;
use crate::cycle::{self, Binding, Outcome, Presses};
use crate::hotkey::Hotkey;
use crate::layout::Placement;

/// Watches for the modifiers of a shortcut under way (waiting for its second
/// stroke, or cycling) to be let go. It runs only between such a press and
/// that release.
pub const MODIFIER_TIMER: usize = 1;
const MODIFIER_POLL_MS: u32 = 15;

pub struct Shortcuts {
    hwnd: HWND,
    bindings: Vec<Binding>,
    presses: Presses,
    /// Hotkey id `n` is `registered[n - 1]`. The first `firsts` of them are
    /// the bindings' first strokes, registered for as long as the bindings
    /// are; the rest are the strokes that may come next, registered while
    /// they may.
    registered: Vec<Hotkey>,
    firsts: usize,
}

impl Shortcuts {
    pub fn new(hwnd: HWND) -> Shortcuts {
        Shortcuts {
            hwnd,
            bindings: Vec::new(),
            presses: Presses::default(),
            registered: Vec::new(),
            firsts: 0,
        }
    }

    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    /// Registers `bindings` in place of the ones before, and returns the
    /// strokes another application or Windows already holds.
    pub fn set(&mut self, bindings: Vec<Binding>) -> Vec<String> {
        self.clear();
        self.bindings = bindings;
        let firsts = cycle::first_strokes(&self.bindings);
        self.firsts = firsts.len();
        self.register(firsts)
    }

    /// Lets go of every shortcut.
    pub fn clear(&mut self) {
        self.reset();
        self.unregister_from(0);
        self.bindings.clear();
        self.firsts = 0;
    }

    /// A WM_HOTKEY with `id`: the placement to apply, when the press
    /// completes a shortcut.
    pub fn on_hotkey(&mut self, id: usize) -> Option<Placement> {
        let stroke = *self.registered.get(id.wrapping_sub(1))?;
        let outcome = self.presses.press(&self.bindings, stroke);
        if self.presses.watching(&self.bindings).is_some() {
            // Restarting a running timer just resets its interval.
            unsafe { SetTimer(Some(self.hwnd), MODIFIER_TIMER, MODIFIER_POLL_MS, None) };
        } else {
            let _ = unsafe { KillTimer(Some(self.hwnd), MODIFIER_TIMER) };
        }
        self.sync_next();
        match outcome {
            Outcome::Place { binding, entry } => Some(self.bindings[binding].placements[entry]),
            Outcome::Pending | Outcome::Nothing => None,
        }
    }

    /// A WM_TIMER. Returns whether it was this one. Letting go of any of
    /// the modifiers counts as letting go: the second stroke is no longer
    /// awaited, and the next press starts from a binding's first entry.
    pub fn on_timer(&mut self, id: usize) -> bool {
        if id != MODIFIER_TIMER {
            return false;
        }
        if self
            .presses
            .watching(&self.bindings)
            .is_none_or(|m| !modifiers_held(m))
        {
            self.reset();
        }
        true
    }

    /// Starts the next press over and stops watching.
    fn reset(&mut self) {
        self.presses.reset();
        let _ = unsafe { KillTimer(Some(self.hwnd), MODIFIER_TIMER) };
        self.sync_next();
    }

    /// Registers the strokes that may come next, and only those, on top of
    /// the first strokes.
    fn sync_next(&mut self) {
        let next = self.presses.next_strokes(&self.bindings);
        if self.registered[self.firsts.min(self.registered.len())..] == next[..] {
            return;
        }
        self.unregister_from(self.firsts);
        // One another application holds simply does not work.
        self.register(next);
    }

    fn register(&mut self, strokes: Vec<Hotkey>) -> Vec<String> {
        let mut failed = Vec::new();
        for h in strokes {
            self.registered.push(h);
            let id = self.registered.len() as i32;
            let modifiers = HOT_KEY_MODIFIERS(h.modifiers) | MOD_NOREPEAT;
            if unsafe { RegisterHotKey(Some(self.hwnd), id, modifiers, h.vk) }.is_err() {
                failed.push(h.to_string());
            }
        }
        failed
    }

    fn unregister_from(&mut self, from: usize) {
        for id in from + 1..=self.registered.len() {
            let _ = unsafe { UnregisterHotKey(Some(self.hwnd), id as i32) };
        }
        self.registered.truncate(from);
    }
}
