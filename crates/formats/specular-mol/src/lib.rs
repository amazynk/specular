//! Molecular structures: our own readers for PDB, PDBx/mmCIF, small-molecule CIF (IUCr core,
//! filled into the unit cell by its symmetry operators), MDL SDF/MOL (V2000 and V3000) and
//! molecular XYZ, Tripos MOL2 and GROMACS GRO, plain or gzip-compressed, into one [`Molecule`]: atoms grouped into
//! residues and chains, bonds (from the file, a standard residue table and by distance),
//! secondary structure (from the file or a DSSP-like assignment), the unit cell and the
//! entry's description.
//!
//! [`cartoon`] builds the protein ribbon and nucleic-acid trace as a triangle mesh,
//! [`Colouring`] colours atoms and residues by element, chain, secondary structure,
//! B-factor or residue index, and [`preview`] renders a small image on the CPU.
//!
//! Every reader is bounded by [`Limits`]: absurd atom counts are refused before anything
//! is allocated for them.

mod bonds;
pub mod cartoon;
pub mod cif;
mod colour;
pub mod element;
mod md;
mod pdb;
pub mod preview;
pub mod scene;
mod sdf;
mod ss;
mod symop;
mod xyz;

#[cfg(test)]
mod tests;

pub use bonds::{RESIDUE_BONDS, bond_by_distance};
pub use colour::{CHAIN_PALETTE, Colouring, Style};
pub use element::Element;
pub use scene::{Ball, Scene, Stick};
pub use symop::SymOp;
pub use xyz::is_molecular_xyz;

/// The formats read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MolFormat {
    Pdb,
    Mmcif,
    /// A small-molecule (IUCr core) CIF with a unit cell.
    Cif,
    Sdf,
    Xyz,
    /// Tripos MOL2.
    Mol2,
    /// GROMACS GRO.
    Gro,
}

impl MolFormat {
    pub fn name(self) -> &'static str {
        match self {
            MolFormat::Pdb => "PDB",
            MolFormat::Mmcif => "PDBx/mmCIF",
            MolFormat::Cif => "CIF",
            MolFormat::Sdf => "SDF",
            MolFormat::Xyz => "XYZ",
            MolFormat::Mol2 => "MOL2",
            MolFormat::Gro => "GROMACS GRO",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("malformed {0}")]
    Malformed(String),
    #[error("too large: {0}")]
    TooLarge(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("no atoms")]
    Empty,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Caps on what a file may make us allocate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Atoms kept (the largest PDB entries have a few million).
    pub max_atoms: usize,
    /// Bytes after decompression.
    pub max_bytes: usize,
    /// Atoms after filling a small-molecule unit cell by symmetry.
    pub max_cell_atoms: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_atoms: 20_000_000,
            max_bytes: 4 << 30,
            max_cell_atoms: 2_000_000,
        }
    }
}

/// A short ASCII name (atom, residue): up to 8 bytes, NUL-padded.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tag([u8; 8]);

impl Tag {
    /// From bytes, trimmed of spaces, truncated to 8, non-ASCII made `?`.
    pub fn new(s: &[u8]) -> Tag {
        let s = s.trim_ascii();
        let mut t = [0u8; 8];
        for (d, &c) in t.iter_mut().zip(s) {
            *d = if c.is_ascii() && c != 0 { c } else { b'?' };
        }
        Tag(t)
    }

    pub fn as_str(&self) -> &str {
        let n = self.0.iter().position(|&c| c == 0).unwrap_or(8);
        std::str::from_utf8(&self.0[..n]).unwrap_or("?")
    }

    pub fn is_empty(&self) -> bool {
        self.0[0] == 0
    }
}

impl std::fmt::Debug for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

impl std::fmt::Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Atom {
    /// Cartesian, Å.
    pub pos: [f32; 3],
    /// Isotropic displacement (B, Å²; from U_iso × 8π² in small-molecule CIFs).
    pub b: f32,
    pub occupancy: f32,
    /// The file's serial number (or 1-based position where it has none).
    pub serial: i32,
    /// Index into [`Molecule::residues`].
    pub residue: u32,
    pub name: Tag,
    pub element: Element,
    /// Alternate location (0: none).
    pub altloc: u8,
    pub charge: i8,
    /// HETATM (or a non-polymer atom in mmCIF).
    pub het: bool,
}

/// Secondary structure of a residue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Ss {
    #[default]
    Coil,
    Helix,
    Strand,
}

/// What a residue is for drawing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ResKind {
    /// Amino acid with a Cα.
    Amino,
    /// Nucleotide with a backbone phosphorus or C4′.
    Nucleic,
    Water,
    /// Ligands, ions and everything else.
    #[default]
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Residue {
    pub name: Tag,
    pub seq: i32,
    /// Insertion code (0: none).
    pub icode: u8,
    /// Index into [`Molecule::chains`].
    pub chain: u32,
    /// Atom range.
    pub first: u32,
    pub end: u32,
    pub ss: Ss,
    pub kind: ResKind,
    /// The atom the cartoon passes through (Cα, P or C4′).
    pub trace: Option<u32>,
}

impl Residue {
    pub fn atoms(&self) -> std::ops::Range<usize> {
        self.first as usize..self.end as usize
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chain {
    /// The author's chain name (PDB chain ID, mmCIF auth_asym_id).
    pub id: String,
    /// Residue range.
    pub first: u32,
    pub end: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bond {
    pub a: u32,
    pub b: u32,
    /// 1, 2, 3; 4 aromatic; 0 unknown (drawn single).
    pub order: u8,
}

/// The unit cell: edges (Å) and angles (degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub alpha: f64,
    pub beta: f64,
    pub gamma: f64,
}

impl Cell {
    /// Fractional to Cartesian (PDB convention: a along x, b in the xy plane), by rows.
    pub fn orth(&self) -> [[f64; 3]; 3] {
        let (ca, cb, cg) = (
            self.alpha.to_radians().cos(),
            self.beta.to_radians().cos(),
            self.gamma.to_radians().cos(),
        );
        let sg = self.gamma.to_radians().sin();
        let v = (1.0 - ca * ca - cb * cb - cg * cg + 2.0 * ca * cb * cg)
            .max(0.0)
            .sqrt();
        [
            [self.a, self.b * cg, self.c * cb],
            [0.0, self.b * sg, self.c * (ca - cb * cg) / sg],
            [0.0, 0.0, self.c * v / sg],
        ]
    }

    pub fn to_cartesian(&self, f: [f64; 3]) -> [f64; 3] {
        let m = self.orth();
        [0, 1, 2].map(|r| m[r][0] * f[0] + m[r][1] * f[1] + m[r][2] * f[2])
    }

    pub fn to_fractional(&self, p: [f64; 3]) -> [f64; 3] {
        let m = self.orth();
        // Upper triangular: back-substitute.
        let z = p[2] / m[2][2];
        let y = (p[1] - m[1][2] * z) / m[1][1];
        let x = (p[0] - m[0][1] * y - m[0][2] * z) / m[0][0];
        [x, y, z]
    }

    pub fn volume(&self) -> f64 {
        let m = self.orth();
        m[0][0] * m[1][1] * m[2][2]
    }

    /// Edges and angles are positive and make a cell (the 1 Å cube EM entries carry is
    /// still a cell; callers decide whether it means anything).
    pub fn is_valid(&self) -> bool {
        [self.a, self.b, self.c]
            .iter()
            .all(|v| v.is_finite() && *v > 0.0)
            && [self.alpha, self.beta, self.gamma]
                .iter()
                .all(|v| v.is_finite() && *v > 0.0 && *v < 180.0)
            && self.volume().is_finite()
            && self.volume() > 0.0
    }

    /// A placeholder cell (1 Å cube) that EM and NMR entries carry.
    pub fn is_placeholder(&self) -> bool {
        self.a <= 1.0 && self.b <= 1.0 && self.c <= 1.0
    }

    /// The cell's eight corners, Cartesian (corner `i` has fractional `i & 1`, `i >> 1 & 1`,
    /// `i >> 2 & 1`).
    pub fn corners(&self) -> [[f32; 3]; 8] {
        std::array::from_fn(|i| {
            let f = [(i & 1) as f64, (i >> 1 & 1) as f64, (i >> 2 & 1) as f64];
            self.to_cartesian(f).map(|v| v as f32)
        })
    }
}

/// Where secondary structure came from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SsSource {
    /// No polymer, or nothing assigned.
    #[default]
    None,
    /// HELIX/SHEET records, struct_conf / struct_sheet_range.
    File,
    /// Our DSSP-like assignment from backbone hydrogen bonds.
    Computed,
}

/// What the file says about the entry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Info {
    /// PDB ID, CIF data block name, SDF/MOL or XYZ title.
    pub id: Option<String>,
    pub title: Option<String>,
    /// Experimental method(s).
    pub method: Option<String>,
    /// Resolution (Å).
    pub resolution: Option<f64>,
    /// Hermann–Mauguin space group.
    pub space_group: Option<String>,
    /// The primary citation: title, journal, year, DOI.
    pub citation: Option<String>,
    /// Chemical formula (small-molecule CIF `_chemical_formula_sum`).
    pub formula: Option<String>,
    /// Classification (PDB HEADER) or compound name.
    pub classification: Option<String>,
    /// Deposition date.
    pub date: Option<String>,
    /// Entity descriptions (mmCIF entity.pdbx_description), polymers first.
    pub entities: Vec<String>,
}

/// One structure, the first model of the file.
#[derive(Clone, Debug, Default)]
pub struct Molecule {
    pub format: Option<MolFormat>,
    pub atoms: Vec<Atom>,
    pub residues: Vec<Residue>,
    pub chains: Vec<Chain>,
    pub bonds: Vec<Bond>,
    /// Bonds the file listed (CONECT, `_geom_bond`, the SDF bond block).
    pub file_bonds: usize,
    pub cell: Option<Cell>,
    /// Symmetry operators the file gave (small-molecule CIF).
    pub symops: Vec<SymOp>,
    /// Atoms in the asymmetric unit before the cell was filled (0 when it was not).
    pub asymmetric_atoms: usize,
    /// Models (PDB MODEL, mmCIF model numbers); the first is read.
    pub models: usize,
    /// Records (SDF), data blocks (CIF) or frames (XYZ); the first is read.
    pub records: usize,
    pub ss_source: SsSource,
    pub info: Info,
    /// What was skipped or approximated, for the info panel.
    pub notes: Vec<String>,
    /// The input was gzip-compressed.
    pub gzipped: bool,
}

/// Bond (stick) radius in ball-and-stick, Å.
pub const BOND_RADIUS: f32 = 0.14;

/// The sphere radius of an atom of element `e` in `style`, Å.
pub fn atom_radius(style: Style, e: Element) -> f32 {
    match style {
        Style::Spacefill => element::vdw(e),
        Style::Cartoon | Style::BallStick => 0.24 * element::vdw(e),
    }
}

/// Formats [`open`] recognises from content.
pub fn sniff(head: &[u8]) -> Option<MolFormat> {
    if head.starts_with(&[0x1F, 0x8B]) {
        return None;
    }
    let text = head;
    if xyz::is_molecular_xyz(text) {
        return Some(MolFormat::Xyz);
    }
    if md::is_mol2(text) {
        return Some(MolFormat::Mol2);
    }
    if md::is_gro(text) {
        return Some(MolFormat::Gro);
    }
    let first_line = |pfx: &[u8]| text.starts_with(pfx);
    if (first_line(b"HEADER ")
        || first_line(b"CRYST1")
        || (first_line(b"ATOM  ") || first_line(b"HETATM")) && text.len() >= 54
        || first_line(b"REMARK ")
        || first_line(b"COMPND ")
        || first_line(b"MODEL ")
        || first_line(b"TITLE ")
        || first_line(b"EXPDTA "))
        && pdb::looks_like_pdb(text)
    {
        return Some(MolFormat::Pdb);
    }
    if let Some(f) = cif::sniff(text) {
        return Some(f);
    }
    if sdf::looks_like_sdf(text) {
        return Some(MolFormat::Sdf);
    }
    None
}

/// Bytes [`sniff_file`] looks at (inflated first when the file is gzip).
pub const SNIFF_HEAD: usize = 4096;

/// [`sniff`] on the start of a whole file, inflating it first when it is gzip-compressed.
pub fn sniff_file(data: &[u8]) -> Option<MolFormat> {
    if data.starts_with(&[0x1F, 0x8B]) {
        let head = specular_png::inflate_gzip_prefix(data, SNIFF_HEAD).ok()?;
        return sniff(&head);
    }
    sniff(&data[..data.len().min(SNIFF_HEAD)])
}

/// Reads `data` (plain or gzip-compressed) as `format`, or as what its content looks like
/// when `format` is `None`.
pub fn open(data: &[u8], format: Option<MolFormat>, limits: &Limits) -> Result<Molecule> {
    let (text, gz, _held) = if data.starts_with(&[0x1F, 0x8B]) {
        let out = specular_png::inflate_gzip_members(data, limits.max_bytes, None).map_err(
            |e| match e {
                specular_png::InflateError::NeedsMemory(n) => Error::TooLarge(n.to_string()),
                e => Error::Malformed(format!("gzip: {e}")),
            },
        )?;
        // The inflater's reservation ends with it; the text lives on through parsing.
        let held = specular_core::budget::reserve(out.len() as u64)
            .map_err(|n| Error::TooLarge(n.to_string()))?;
        (std::borrow::Cow::Owned(out), true, Some(held))
    } else {
        (std::borrow::Cow::Borrowed(data), false, None)
    };
    let format = match format {
        Some(f) => f,
        None => sniff(&text[..text.len().min(SNIFF_HEAD)])
            .ok_or_else(|| Error::Unsupported("not a molecular structure".into()))?,
    };
    let mut m = match format {
        MolFormat::Pdb => pdb::read(&text, limits)?,
        MolFormat::Mmcif | MolFormat::Cif => cif::read(&text, limits)?,
        MolFormat::Sdf => sdf::read(&text, limits)?,
        MolFormat::Xyz => xyz::read(&text, limits)?,
        MolFormat::Mol2 => md::read_mol2(&text, limits)?,
        MolFormat::Gro => md::read_gro(&text, limits)?,
    };
    m.gzipped = gz;
    if m.atoms.is_empty() {
        return Err(Error::Empty);
    }
    m.finish();
    Ok(m)
}

/// One unit-cell edge: its two ends (Cartesian) and its colour.
pub type CellEdge = ([f32; 3], [f32; 3], [u8; 4]);

impl Molecule {
    /// Classifies residues, finds trace atoms, assigns secondary structure where the file
    /// had none, and bonds what the file did not.
    pub(crate) fn finish(&mut self) {
        bonds::classify(self);
        if self.ss_source == SsSource::None
            && self.residues.iter().any(|r| r.kind == ResKind::Amino)
            && ss::assign(self)
        {
            self.ss_source = SsSource::Computed;
        }
        bonds::complete(self);
    }

    /// Replaces the secondary structure with our DSSP-like assignment from the backbone
    /// hydrogen bonds; false when there is no backbone to assign from.
    pub fn compute_secondary_structure(&mut self) -> bool {
        for r in &mut self.residues {
            r.ss = Ss::Coil;
        }
        let any = ss::assign(self);
        self.ss_source = if any {
            SsSource::Computed
        } else {
            SsSource::None
        };
        any
    }

    pub fn bounds(&self) -> specular_mesh::Bounds {
        let mut b = specular_mesh::Bounds::EMPTY;
        for a in &self.atoms {
            b.add(a.pos);
        }
        if let Some(c) = self.cell.filter(|_| self.format == Some(MolFormat::Cif)) {
            for p in c.corners() {
                b.add(p);
            }
        }
        b
    }

    /// The style a file opens in: the cartoon when there is a polymer to draw, else
    /// ball-and-stick.
    pub fn default_style(&self) -> Style {
        if self.has_cartoon() {
            Style::Cartoon
        } else {
            Style::BallStick
        }
    }

    /// There is a polymer segment of two or more residues.
    pub fn has_cartoon(&self) -> bool {
        !cartoon::segments(self).is_empty()
    }

    /// The atoms drawn as spheres in `style`: everything, except that the cartoon replaces
    /// the polymer it covers and hides waters.
    pub fn visible_atoms(&self, style: Style) -> Vec<bool> {
        if style != Style::Cartoon {
            return vec![true; self.atoms.len()];
        }
        let mut in_cartoon = vec![false; self.residues.len()];
        for s in cartoon::segments(self) {
            for r in s {
                in_cartoon[r] = true;
            }
        }
        self.atoms
            .iter()
            .map(|a| {
                let r = a.residue as usize;
                !in_cartoon[r] && self.residues[r].kind != ResKind::Water
            })
            .collect()
    }

    /// The unit cell's twelve edges (Cartesian), with their colours: a red, b green, c blue,
    /// the rest grey. `None` without a meaningful cell.
    pub fn cell_edges(&self) -> Option<[CellEdge; 12]> {
        let c = self.cell.filter(|c| c.is_valid() && !c.is_placeholder())?;
        let k = c.corners();
        const E: [(usize, usize); 12] = [
            (0, 1),
            (0, 2),
            (0, 4),
            (1, 3),
            (1, 5),
            (2, 3),
            (2, 6),
            (3, 7),
            (4, 5),
            (4, 6),
            (5, 7),
            (6, 7),
        ];
        Some(std::array::from_fn(|i| {
            let (a, b) = E[i];
            let col = match i {
                0 => [230, 60, 60, 255],
                1 => [60, 200, 80, 255],
                2 => [70, 120, 240, 255],
                _ => [150, 150, 150, 255],
            };
            (k[a], k[b], col)
        }))
    }

    /// Radius of the cell edges: thin against the cell, visible at any zoom that shows it.
    pub fn cell_edge_radius(&self) -> f32 {
        let Some(c) = self.cell else {
            return 0.05;
        };
        (0.02 * c.a.max(c.b).max(c.c) as f32).clamp(0.03, 0.3)
    }

    /// Residues of the polymer kinds (amino acids and nucleotides).
    pub fn polymer_residues(&self) -> usize {
        self.residues
            .iter()
            .filter(|r| matches!(r.kind, ResKind::Amino | ResKind::Nucleic))
            .count()
    }

    /// The distinct ligand codes (non-polymer, non-water residues), sorted.
    pub fn ligands(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .residues
            .iter()
            .filter(|r| r.kind == ResKind::Other)
            .map(|r| r.name.as_str().to_owned())
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// The residue's chain.
    pub fn chain_of(&self, r: &Residue) -> &Chain {
        &self.chains[r.chain as usize]
    }

    pub fn memory_bytes(&self) -> usize {
        self.atoms.len() * size_of::<Atom>()
            + self.residues.len() * size_of::<Residue>()
            + self.bonds.len() * size_of::<Bond>()
            + self.chains.len() * 48
    }

    /// The chain kind summary: protein, nucleic-acid and other chains.
    pub fn chain_kinds(&self) -> (usize, usize, usize) {
        let (mut p, mut n, mut o) = (0, 0, 0);
        for c in &self.chains {
            let rs = &self.residues[c.first as usize..c.end as usize];
            let aa = rs.iter().filter(|r| r.kind == ResKind::Amino).count();
            let na = rs.iter().filter(|r| r.kind == ResKind::Nucleic).count();
            if aa > 0 && aa >= na {
                p += 1;
            } else if na > 0 {
                n += 1;
            } else {
                o += 1;
            }
        }
        (p, n, o)
    }
}

/// Builder shared by the readers: appends atoms, opening residues and chains as the keys
/// change.
pub(crate) struct Build {
    pub m: Molecule,
    limits: Limits,
    /// Chain id, and whether a TER (or a new chain) closed it.
    chain_open: bool,
    /// Key of the open residue: (seq, icode, name, extra discriminator).
    res_key: Option<(i32, u8, Tag, u32)>,
    /// The open residue's first alternate location.
    res_alt: u8,
}

pub(crate) struct AtomIn<'a> {
    pub chain: &'a [u8],
    pub res_name: Tag,
    pub seq: i32,
    pub icode: u8,
    /// Separates residues that share number and name (mmCIF label_asym_id).
    pub sub: u32,
    pub atom: Atom,
}

impl Build {
    pub fn new(format: MolFormat, limits: &Limits) -> Build {
        Build {
            m: Molecule {
                format: Some(format),
                models: 1,
                records: 1,
                ..Default::default()
            },
            limits: *limits,
            chain_open: false,
            res_key: None,
            res_alt: 0,
        }
    }

    /// Closes the current chain (PDB TER).
    pub fn ter(&mut self) {
        self.chain_open = false;
        self.res_key = None;
    }

    /// Adds an atom; `false` when its alternate location is not the first (skipped).
    pub fn push(&mut self, a: AtomIn) -> Result<bool> {
        let new_chain = !self.chain_open
            || self
                .m
                .chains
                .last()
                .is_none_or(|c| c.id.as_bytes() != a.chain.trim_ascii());
        let key = (a.seq, a.icode, a.res_name, a.sub);
        let same_place = !new_chain
            && self
                .res_key
                .is_some_and(|k| k.0 == key.0 && k.1 == key.1 && k.3 == key.3);
        if same_place && a.atom.altloc != 0 {
            // Alternate conformers (and microheterogeneity): keep the residue's first.
            if self.res_alt == 0 {
                self.res_alt = a.atom.altloc;
            } else if a.atom.altloc != self.res_alt {
                return Ok(false);
            }
        }
        if new_chain || self.res_key != Some(key) {
            if new_chain {
                let first = self.m.residues.len() as u32;
                self.m.chains.push(Chain {
                    id: String::from_utf8_lossy(a.chain.trim_ascii()).into_owned(),
                    first,
                    end: first,
                });
                self.chain_open = true;
            }
            let at = self.m.atoms.len() as u32;
            self.m.residues.push(Residue {
                name: a.res_name,
                seq: a.seq,
                icode: a.icode,
                chain: self.m.chains.len() as u32 - 1,
                first: at,
                end: at,
                ss: Ss::Coil,
                kind: ResKind::Other,
                trace: None,
            });
            if let Some(c) = self.m.chains.last_mut() {
                c.end += 1;
            }
            self.res_key = Some(key);
            self.res_alt = a.atom.altloc;
        }
        if self.m.atoms.len() >= self.limits.max_atoms {
            return Err(Error::TooLarge(format!(
                "more than {} atoms",
                self.limits.max_atoms
            )));
        }
        let mut atom = a.atom;
        atom.residue = self.m.residues.len() as u32 - 1;
        self.m.atoms.push(atom);
        if let Some(r) = self.m.residues.last_mut() {
            r.end += 1;
        }
        Ok(true)
    }

    /// Reserves for `n` atoms (refusing more than the limit allows).
    pub fn reserve(&mut self, n: usize) -> Result<()> {
        if n > self.limits.max_atoms {
            return Err(Error::TooLarge(format!(
                "{n} atoms (limit {})",
                self.limits.max_atoms
            )));
        }
        self.m.atoms.reserve(n);
        Ok(())
    }
}

/// Parses a decimal (`-12.345`, `1e-3`, `7`) quickly; `None` when it is not a number.
pub(crate) fn num(s: &[u8]) -> Option<f64> {
    let s = s.trim_ascii();
    let (neg, body) = match s.first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    if body.is_empty() || body.len() > 18 {
        return slow_num(s);
    }
    let mut int: u64 = 0;
    let mut frac_digits = 0u32;
    let mut seen_dot = false;
    let mut digits = 0;
    for &c in body {
        match c {
            b'0'..=b'9' => {
                int = int * 10 + u64::from(c - b'0');
                digits += 1;
                if seen_dot {
                    frac_digits += 1;
                }
            }
            b'.' if !seen_dot => seen_dot = true,
            _ => return slow_num(s),
        }
    }
    if digits == 0 {
        return None;
    }
    const P10: [f64; 19] = [
        1.0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16,
        1e17, 1e18,
    ];
    let v = int as f64 / P10[frac_digits as usize];
    Some(if neg { -v } else { v })
}

fn slow_num(s: &[u8]) -> Option<f64> {
    // CIF numbers may carry a standard uncertainty: 1.234(5).
    let s = match s.iter().position(|&c| c == b'(') {
        Some(i) => &s[..i],
        None => s,
    };
    let v: f64 = std::str::from_utf8(s).ok()?.trim().parse().ok()?;
    v.is_finite().then_some(v)
}

pub(crate) fn int(s: &[u8]) -> Option<i64> {
    let s = s.trim_ascii();
    let (neg, body) = match s.first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    if body.is_empty() || body.len() > 18 || !body.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let v = body.iter().fold(0i64, |a, &c| a * 10 + i64::from(c - b'0'));
    Some(if neg { -v } else { v })
}
