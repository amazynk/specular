//! Diagnostics that stay on the computer until the user shares them.
//!
//! - [`redact`]: removes the home folder, the user name and (by default) file and folder
//!   names from crash reports, logs and settings.
//! - [`bundle`]: the diagnostics bundle, a zip the user previews and saves where they choose.
//!   Nothing is ever uploaded; the user sends it themselves ([`ISSUES_URL`], [`SUPPORT_EMAIL`]).
//! - [`update`]: the opt-in update check (off by default): one plain GET of [`RELEASES_URL`]
//!   at most once a day, no identifiers, no cookies, user agent `Specular/<version>`.

pub mod bundle;
pub mod redact;
pub mod update;
pub mod zip;

pub use bundle::{Entry, Inputs, What};
pub use redact::Redactor;

/// The release feed the update check reads (the format is in [`update`]), served by the
/// website alongside the Specular page.
pub const RELEASES_URL: &str = "https://amazynk.com/specular/releases.json";

/// Where users report problems.
pub const ISSUES_URL: &str = "https://github.com/amazynk/specular/issues";

/// PLACEHOLDER: the support address. Replace before release.
pub const SUPPORT_EMAIL: &str = "support@example.com";

/// The version this build reports.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
