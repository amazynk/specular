//! Collected parameter sets and the sequence/picture lookup they provide.

use crate::error::{Error, Result};
use crate::nal::{NalHeader, Rbsp, unit_type};
use crate::ps::{Pps, Sps, Vps, parse_sps, pps, vps};
use alloc::vec::Vec;

/// All parameter sets supplied to a decode call.
#[derive(Debug, Clone, Default)]
pub struct ParameterSets {
    /// Video parameter sets, kept only so callers can see they parsed.
    pub vps: Vec<Vps>,
    /// Sequence parameter sets.
    pub sps: Vec<Sps>,
    /// Picture parameter sets.
    pub pps: Vec<Pps>,
    /// Why the first parameter set that failed to parse was dropped; it is
    /// reported if a lookup then finds nothing.
    pub dropped: Option<Error>,
}

impl ParameterSets {
    /// Parses a list of headerless NAL units into their parameter sets.
    ///
    /// A set that does not parse is dropped rather than failing the call: a
    /// damaged VPS is never needed, and a missing SPS or PPS is reported by
    /// the lookup, with the parse error that lost it.
    pub fn parse(sets: &[&[u8]]) -> Result<ParameterSets> {
        let mut out = ParameterSets::default();
        for nal in sets {
            if let Err(e) = out.add(nal) {
                // An unsupported tool is the more useful report.
                if out.dropped.is_none() || matches!(e, Error::Unsupported(_)) {
                    out.dropped = Some(e);
                }
            }
        }
        Ok(out)
    }

    /// Parses one parameter set NAL unit and keeps it (replacing an earlier
    /// one with the same id). Other NAL units are ignored.
    pub(crate) fn add(&mut self, nal: &[u8]) -> Result<()> {
        let h = NalHeader::parse(nal)?;
        if h.layer_id != 0 || !(unit_type::VPS..=unit_type::PPS).contains(&h.nal_unit_type) {
            return Ok(());
        }
        let rbsp = Rbsp::from_nal(nal)?;
        match h.nal_unit_type {
            unit_type::VPS => {
                let v = vps::parse(&rbsp.data)?;
                self.vps.retain(|e| e.id != v.id);
                self.vps.push(v);
            }
            unit_type::SPS => {
                let s = parse_sps(&rbsp.data)?;
                self.sps.retain(|e| e.id != s.id);
                self.sps.push(s);
            }
            _ => {
                let sps = &self.sps;
                let p = pps::parse(&rbsp.data, |id| {
                    sps.iter().any(|s| s.id == id && s.v1_profile)
                })?;
                self.pps.retain(|e| e.id != p.id);
                self.pps.push(p);
            }
        }
        Ok(())
    }

    /// Looks up a picture parameter set by id.
    pub fn pps(&self, id: u32) -> Result<&Pps> {
        self.pps
            .iter()
            .find(|p| p.id == id)
            .ok_or(self.dropped.unwrap_or(Error::MissingParameterSet("PPS")))
    }

    /// Looks up a sequence parameter set by id.
    pub fn sps(&self, id: u32) -> Result<&Sps> {
        self.sps
            .iter()
            .find(|s| s.id == id)
            .ok_or(self.dropped.unwrap_or(Error::MissingParameterSet("SPS")))
    }

    /// The sequence parameter set the first picture parameter set refers to.
    pub fn first_sps(&self) -> Result<&Sps> {
        match self.pps.first() {
            Some(p) => self.sps(p.sps_id),
            None => self
                .sps
                .first()
                .ok_or(self.dropped.unwrap_or(Error::MissingParameterSet("SPS"))),
        }
    }
}
