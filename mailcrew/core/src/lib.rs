//! What Mail Crew does, apart from the screens that show it: signing in to
//! Google and keeping the accounts in the OS credential store, reading and
//! changing mail through the Gmail API, turning messages into cards and
//! sanitised bodies, and the sender checks under a card's subject.
//!
//! The native macOS app uses it through the bindings in `ffi` (the `ffi`
//! feature), which wrap `Core`.

pub mod accounts;
pub mod attachments;
pub mod auth;
pub mod authres;
pub mod bimi;
pub mod card;
pub mod credentials;
pub mod error;
pub mod gmail;
pub mod image_senders;
pub mod links;
pub mod mime;
pub mod preferences;
pub mod sanitize;
mod service;
pub mod views;

#[cfg(feature = "ffi")]
mod ffi;

pub use service::{Core, PendingSignIn};

#[cfg(feature = "ffi")]
uniffi::setup_scaffolding!();
