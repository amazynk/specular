//! Diagnostics that stay on the computer until the user shares them.
//!
//! - [`redact`]: removes the home folder, the user name and (by default) file and folder
//!   names from crash reports, logs and settings.
//! - [`bundle`]: the diagnostics bundle, a zip the user previews and saves where they choose.
//!   Nothing is ever uploaded; the user sends it themselves ([`ISSUES_URL`], [`SUPPORT_EMAIL`]).
//! - [`update`] checks GitHub Releases on demand or automatically when enabled. Each check
//!   sends the user agent `Specular/<version>` without identifiers or cookies.

pub mod bundle;
pub mod redact;
pub mod update;
pub mod zip;

pub use bundle::{Entry, Inputs, What};
pub use redact::Redactor;

/// Where users report problems.
pub const ISSUES_URL: &str = "https://github.com/amazynk/specular/issues";

/// PLACEHOLDER: the support address. Replace before release.
pub const SUPPORT_EMAIL: &str = "support@example.com";

/// The version this build reports.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
