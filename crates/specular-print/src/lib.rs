//! Printing and contact sheets.
//!
//! [`layout`] places images on sheets of paper (one per page, fitted, filled or at a given
//! size, or a contact sheet grid with captions and a header); [`render`] rasterises the
//! pages on the GPU through the viewer's own image renderer, a band at a time; [`pdf`]
//! streams the bands into a PDF with an sRGB output intent ([`icc`]). The PDF is what the
//! platform's print service receives, or what "Save as PDF" keeps. [`worker`] runs it all
//! on a thread of its own. [`profile`] prints through a printer's ICC profile. [`free`]
//! places photos in image areas laid out by hand, and saves such layouts by name.

pub mod files;
pub mod free;
pub mod icc;
pub mod layout;
pub mod pdf;
pub mod profile;
pub mod render;
pub mod worker;

pub use layout::{
    Caption, Document, Fit, Item, Kind, Page, PageOrientation, PageSetup, Paper, RectMm, Unit,
    layout,
};
pub use render::{ImageSource, Loaded, PageRenderer, PageStats, RenderError};
pub use worker::{PdfDone, PreviewFilter, Reply, Worker};
