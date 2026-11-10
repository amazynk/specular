//! Geodesy for maps, on the WGS 84 ellipsoid only (other datums and projections are the
//! map's CRS's business, see [`seam`]).
//!
//! - [`coords`]: coordinates as text in four formats (decimal degrees, degrees minutes
//!   seconds, UTM/UPS, MGRS/USNG) and a parser that reads all of them.
//! - [`utm`], [`mgrs`]: Transverse Mercator (Krüger series to sixth order, nanometre
//!   accuracy within a zone), polar stereographic for UPS, and the MGRS letters.
//! - [`geodesic`]: distances, azimuths, positions along a line and polygon areas after
//!   Karney (2013), exact to round-off.
//! - [`units`]: metric and imperial lengths and areas, and "nice" scale-bar lengths.
//! - [`profile`]: an elevation profile along a path over any height function.
//! - [`terrain`]: slope, aspect and contour lines of an elevation grid.
//! - [`time`], [`track`]: ISO 8601 and EXIF times, and positions along a GPS track in time.
//! - [`seam`]: the traits a georeferenced image implements so all of the above can work on
//!   its stored pixels.
//!
//! Nothing here allocates more than its output, touches files or the network.

pub mod coords;
pub mod geodesic;
pub mod mgrs;
pub mod profile;
pub mod seam;
pub mod terrain;
pub mod time;
pub mod track;
pub mod units;
pub mod utm;

mod angle;

/// WGS 84 semi-major axis, metres.
pub const WGS84_A: f64 = 6_378_137.0;
/// WGS 84 flattening.
pub const WGS84_F: f64 = 1.0 / 298.257_223_563;
