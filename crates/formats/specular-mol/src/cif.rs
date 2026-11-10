//! Our own CIF/STAR tokenizer (CIF 1.1, and CIF 2.0's triple-quoted strings), and the two
//! readers on it: PDBx/mmCIF (atom_site streamed row by row into the structure, so a
//! million-atom file is never held as strings) and small-molecule CIF (IUCr core: the cell,
//! fractional sites and the symmetry operators that fill the cell).
//!
//! Tag names are compared lowercased with `.` read as `_`, so DDL1 (`_cell_length_a`) and
//! DDL2 (`_cell.length_a`) spellings of the same item match.

use std::collections::HashMap;

use crate::symop::SymOp;
use crate::{
    Atom, AtomIn, Bond, Build, Cell, Error, Limits, MolFormat, Molecule, Result, Ss, SsSource, Tag,
    element, int, num,
};

/// A value as written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Val<'a> {
    Plain(&'a [u8]),
    Quoted(&'a [u8]),
    /// A semicolon-delimited text field (the lines between the delimiters).
    Text(&'a [u8]),
}

impl<'a> Val<'a> {
    pub fn bytes(&self) -> &'a [u8] {
        match *self {
            Val::Plain(b) | Val::Quoted(b) | Val::Text(b) => b,
        }
    }

    /// `.` (inapplicable) or `?` (unknown), unquoted.
    pub fn is_null(&self) -> bool {
        matches!(self, Val::Plain(b"." | b"?"))
    }

    pub fn text(&self) -> Option<String> {
        (!self.is_null()).then(|| String::from_utf8_lossy(self.bytes().trim_ascii()).into_owned())
    }
}

/// A token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tok<'a> {
    /// `data_NAME`.
    Data(&'a [u8]),
    Loop,
    /// `save_NAME` (empty name: the frame's end).
    Save(&'a [u8]),
    Global,
    Stop,
    Tag(&'a [u8]),
    Value(Val<'a>),
}

/// Splits CIF text into tokens.
pub struct Tokenizer<'a> {
    s: &'a [u8],
    i: usize,
    line: usize,
}

impl<'a> Tokenizer<'a> {
    pub fn new(s: &'a [u8]) -> Self {
        Tokenizer { s, i: 0, line: 1 }
    }

    /// The current line (1-based), for messages.
    pub fn line(&self) -> usize {
        self.line
    }

    fn err(&self, what: &str) -> Error {
        Error::Malformed(format!("CIF line {}: {what}", self.line))
    }

    pub fn next_token(&mut self) -> Result<Option<Tok<'a>>> {
        let s = self.s;
        // Whitespace and comments.
        while self.i < s.len() {
            match s[self.i] {
                b'\n' => {
                    self.line += 1;
                    self.i += 1;
                }
                b' ' | b'\t' | b'\r' | b'\x0B' | b'\x0C' => self.i += 1,
                b'#' => {
                    while self.i < s.len() && s[self.i] != b'\n' {
                        self.i += 1;
                    }
                }
                _ => break,
            }
        }
        if self.i >= s.len() {
            return Ok(None);
        }
        let start = self.i;
        let at_line_start = start == 0 || s[start - 1] == b'\n' || s[start - 1] == b'\r';
        let c = s[start];
        if c == b';' && at_line_start {
            return self.text_field().map(Some);
        }
        if c == b'\'' || c == b'"' {
            return self.quoted(c).map(Some);
        }
        while self.i < s.len() && !s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
        let t = &s[start..self.i];
        if t[0] == b'_' {
            return Ok(Some(Tok::Tag(t)));
        }
        if t.len() >= 5 {
            let head = [t[0], t[1], t[2], t[3], t[4]].map(|b| b.to_ascii_lowercase());
            match &head {
                b"data_" => return Ok(Some(Tok::Data(&t[5..]))),
                b"loop_" if t.len() == 5 => return Ok(Some(Tok::Loop)),
                b"save_" => return Ok(Some(Tok::Save(&t[5..]))),
                b"stop_" if t.len() == 5 => return Ok(Some(Tok::Stop)),
                _ => {}
            }
            if t.len() == 7 && t.eq_ignore_ascii_case(b"global_") {
                return Ok(Some(Tok::Global));
            }
        }
        Ok(Some(Tok::Value(Val::Plain(t))))
    }

    fn quoted(&mut self, q: u8) -> Result<Tok<'a>> {
        let s = self.s;
        let start = self.i;
        // CIF 2.0 triple quotes.
        if s[start..].starts_with(&[q, q, q]) {
            let body = start + 3;
            let mut j = body;
            while j + 3 <= s.len() {
                if s[j] == q && s[j + 1] == q && s[j + 2] == q {
                    self.line += s[body..j].iter().filter(|&&c| c == b'\n').count();
                    self.i = j + 3;
                    return Ok(Tok::Value(Val::Quoted(&s[body..j])));
                }
                j += 1;
            }
            return Err(self.err("unterminated triple-quoted string"));
        }
        let mut j = start + 1;
        while j < s.len() {
            let c = s[j];
            if c == b'\n' || c == b'\r' {
                return Err(self.err("quoted string runs past the end of the line"));
            }
            if c == q && (j + 1 == s.len() || s[j + 1].is_ascii_whitespace()) {
                self.i = j + 1;
                return Ok(Tok::Value(Val::Quoted(&s[start + 1..j])));
            }
            j += 1;
        }
        Err(self.err("unterminated quoted string"))
    }

    fn text_field(&mut self) -> Result<Tok<'a>> {
        let s = self.s;
        let body = self.i + 1;
        let mut j = body;
        loop {
            let Some(nl) = s[j..].iter().position(|&c| c == b'\n') else {
                return Err(self.err("unterminated text field"));
            };
            let next = j + nl + 1;
            self.line += 1;
            if s.get(next) == Some(&b';') {
                // The value ends before the newline that precedes the closing semicolon.
                let mut end = j + nl;
                if end > body && s[end - 1] == b'\r' {
                    end -= 1;
                }
                self.i = next + 1;
                let v = &s[body..end.max(body)];
                return Ok(Tok::Value(Val::Text(v)));
            }
            j = next;
        }
    }
}

/// What receives the parse.
pub trait Sink<'a> {
    /// A data block begins; return false to stop parsing.
    fn block(&mut self, name: &'a [u8]) -> Result<bool>;
    fn item(&mut self, tag: &'a [u8], v: Val<'a>) -> Result<()>;
    fn loop_start(&mut self, tags: &[&'a [u8]]) -> Result<()>;
    fn row(&mut self, values: &[Val<'a>]) -> Result<()>;
    fn loop_end(&mut self) -> Result<()>;
}

/// Parses `s` into `sink`.
pub fn parse<'a>(s: &'a [u8], sink: &mut impl Sink<'a>) -> Result<()> {
    let mut t = Tokenizer::new(s);
    let mut pending: Option<Tok<'a>> = None;
    let mut tags: Vec<&'a [u8]> = Vec::new();
    let mut row: Vec<Val<'a>> = Vec::new();
    loop {
        let tok = match pending.take() {
            Some(x) => x,
            None => match t.next_token()? {
                Some(x) => x,
                None => return Ok(()),
            },
        };
        match tok {
            Tok::Data(name) => {
                if !sink.block(name)? {
                    return Ok(());
                }
            }
            Tok::Global | Tok::Save(_) | Tok::Stop => {}
            Tok::Tag(tag) => match t.next_token()? {
                Some(Tok::Value(v)) => sink.item(tag, v)?,
                _ => return Err(t.err("a tag without a value")),
            },
            Tok::Value(_) => return Err(t.err("a value without a tag")),
            Tok::Loop => {
                tags.clear();
                let first = loop {
                    match t.next_token()? {
                        Some(Tok::Tag(tag)) => tags.push(tag),
                        other => break other,
                    }
                };
                if tags.is_empty() {
                    return Err(t.err("loop_ without tags"));
                }
                sink.loop_start(&tags)?;
                row.clear();
                let mut next = first;
                loop {
                    match next {
                        Some(Tok::Value(v)) => {
                            row.push(v);
                            if row.len() == tags.len() {
                                sink.row(&row)?;
                                row.clear();
                            }
                        }
                        other => {
                            if !row.is_empty() {
                                return Err(t.err("a loop's values are not a whole number of rows"));
                            }
                            sink.loop_end()?;
                            pending = other;
                            break;
                        }
                    }
                    next = t.next_token()?;
                }
                if pending.is_none() {
                    return Ok(());
                }
            }
        }
    }
}

/// `_Cat.item` and `_cat_item` alike: lowercase, `.` as `_`.
pub fn norm(tag: &[u8]) -> String {
    tag.iter()
        .map(|&c| {
            let c = c.to_ascii_lowercase();
            if c == b'.' { '_' } else { char::from(c) }
        })
        .collect()
}

/// A stored loop (or the items of a category).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    pub tags: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
}

impl Table {
    pub fn col(&self, tag: &str) -> Option<usize> {
        self.tags.iter().position(|t| t == tag)
    }

    /// The value at `row` of the column `tag`.
    pub fn get(&self, row: usize, tag: &str) -> Option<&str> {
        let c = self.col(tag)?;
        self.rows.get(row)?.get(c)?.as_deref()
    }
}

/// One data block as kept: its items and the loops of the categories we read.
#[derive(Clone, Debug, Default)]
pub struct Block {
    pub name: String,
    pub items: HashMap<String, String>,
    pub loops: Vec<Table>,
}

impl Block {
    /// An item, or the first row of a loop that holds it.
    pub fn value(&self, tag: &str) -> Option<&str> {
        if let Some(v) = self.items.get(tag) {
            return Some(v);
        }
        self.loops.iter().find_map(|l| l.get(0, tag))
    }

    pub fn number(&self, tag: &str) -> Option<f64> {
        self.value(tag).and_then(|v| num(v.as_bytes()))
    }

    /// The loop holding `tag`, or a one-row table of the items with its prefix.
    pub fn table(&self, tag: &str, prefix: &str) -> Option<Table> {
        if let Some(l) = self.loops.iter().find(|l| l.col(tag).is_some()) {
            return Some(l.clone());
        }
        let mut keys: Vec<&String> = self
            .items
            .keys()
            .filter(|k| k.starts_with(prefix))
            .collect();
        if !keys.iter().any(|k| *k == tag) {
            return None;
        }
        keys.sort();
        Some(Table {
            tags: keys.iter().map(|k| (*k).clone()).collect(),
            rows: vec![keys.iter().map(|k| self.items.get(*k).cloned()).collect()],
        })
    }
}

/// Categories whose loops are kept (everything else in a loop is tokenized and dropped).
const KEPT: &[&str] = &[
    "_struct_conf_",
    "_struct_sheet_range_",
    "_symmetry_",
    "_space_group_",
    "_cell_",
    "_exptl_",
    "_refine_",
    "_reflns_",
    "_entity_",
    "_struct_keywords_",
    "_struct_",
    "_citation_",
    "_entry_",
    "_geom_bond_",
    "_atom_site_",
    "_atom_type_",
    "_em_3d_reconstruction_",
    "_journal_",
    "_publ_",
    "_chemical_",
    "_database_",
    "_pdbx_database_status_",
];

const DROPPED: &[&str] = &[
    "_atom_site_aniso",
    "_entity_poly_seq_",
    "_entity_src",
    "_struct_conn",
    "_struct_site",
    "_struct_ref",
    "_struct_asym_",
    "_struct_mon",
    "_publ_author_",
    "_citation_author_",
    "_citation_editor_",
    "_struct_sheet_order",
    "_struct_sheet_hbond",
];

fn kept(tag: &str) -> bool {
    KEPT.iter().any(|p| tag.starts_with(p)) && !DROPPED.iter().any(|p| tag.starts_with(p))
}

/// Columns of an mmCIF atom_site loop.
#[derive(Default)]
struct AtomCols {
    group: Option<usize>,
    id: Option<usize>,
    symbol: Option<usize>,
    label_atom: Option<usize>,
    auth_atom: Option<usize>,
    alt: Option<usize>,
    label_comp: Option<usize>,
    auth_comp: Option<usize>,
    label_asym: Option<usize>,
    auth_asym: Option<usize>,
    label_seq: Option<usize>,
    auth_seq: Option<usize>,
    icode: Option<usize>,
    xyz: [usize; 3],
    occ: Option<usize>,
    b: Option<usize>,
    charge: Option<usize>,
    model: Option<usize>,
}

impl AtomCols {
    fn of(tags: &[String]) -> Option<AtomCols> {
        let f = |n: &str| tags.iter().position(|t| t == n);
        Some(AtomCols {
            group: f("_atom_site_group_pdb"),
            id: f("_atom_site_id"),
            symbol: f("_atom_site_type_symbol"),
            label_atom: f("_atom_site_label_atom_id"),
            auth_atom: f("_atom_site_auth_atom_id"),
            alt: f("_atom_site_label_alt_id"),
            label_comp: f("_atom_site_label_comp_id"),
            auth_comp: f("_atom_site_auth_comp_id"),
            label_asym: f("_atom_site_label_asym_id"),
            auth_asym: f("_atom_site_auth_asym_id"),
            label_seq: f("_atom_site_label_seq_id"),
            auth_seq: f("_atom_site_auth_seq_id"),
            icode: f("_atom_site_pdbx_pdb_ins_code"),
            xyz: [
                f("_atom_site_cartn_x")?,
                f("_atom_site_cartn_y")?,
                f("_atom_site_cartn_z")?,
            ],
            occ: f("_atom_site_occupancy"),
            b: f("_atom_site_b_iso_or_equiv"),
            charge: f("_atom_site_pdbx_formal_charge"),
            model: f("_atom_site_pdbx_pdb_model_num"),
        })
    }
}

/// Collects a CIF into blocks, streaming an mmCIF atom_site loop into a [`Build`].
struct Collector<'l> {
    blocks: Vec<Block>,
    limits: &'l Limits,
    /// Stop at the next block once one holds atoms.
    have_atoms: bool,
    /// The loop being stored, if kept.
    table: Option<Table>,
    /// mmCIF atom rows go here.
    build: Option<Build>,
    cols: Option<AtomCols>,
    first_model: Option<i64>,
    last_model: Option<i64>,
    models: usize,
    rows: usize,
    serial: i32,
    /// label_asym_id strings to small numbers (residue separation).
    subs: HashMap<Vec<u8>, u32>,
    total_blocks: usize,
}

impl<'a> Sink<'a> for Collector<'_> {
    fn block(&mut self, name: &'a [u8]) -> Result<bool> {
        self.total_blocks += 1;
        if self.have_atoms {
            return Ok(false);
        }
        if self.blocks.len() >= 4096 {
            return Err(Error::TooLarge("more than 4096 data blocks".into()));
        }
        self.blocks.push(Block {
            name: String::from_utf8_lossy(name).into_owned(),
            ..Default::default()
        });
        Ok(true)
    }

    fn item(&mut self, tag: &'a [u8], v: Val<'a>) -> Result<()> {
        if self.blocks.is_empty() {
            self.blocks.push(Block::default());
        }
        let b = self.blocks.last_mut().expect("pushed");
        if let Some(text) = v.text() {
            let key = norm(tag);
            if key.starts_with("_atom_site_fract_") || key.starts_with("_atom_site_cartn_") {
                self.have_atoms = true;
            }
            b.items.entry(key).or_insert(text);
        }
        Ok(())
    }

    fn loop_start(&mut self, tags: &[&'a [u8]]) -> Result<()> {
        if self.blocks.is_empty() {
            self.blocks.push(Block::default());
        }
        let names: Vec<String> = tags.iter().map(|t| norm(t)).collect();
        let is_sites = names
            .iter()
            .all(|t| t.starts_with("_atom_site_") && !t.starts_with("_atom_site_aniso"));
        if is_sites
            && self.build.is_none()
            && let Some(cols) = AtomCols::of(&names)
        {
            self.cols = Some(cols);
            self.build = Some(Build::new(MolFormat::Mmcif, self.limits));
            self.have_atoms = true;
            return Ok(());
        }
        if is_sites && names.iter().any(|t| t == "_atom_site_fract_x") {
            self.have_atoms = true;
        }
        self.table = kept(&names[0]).then(|| Table {
            tags: names,
            rows: Vec::new(),
        });
        Ok(())
    }

    fn row(&mut self, values: &[Val<'a>]) -> Result<()> {
        if let Some(cols) = &self.cols {
            let b = self.build.as_mut().expect("with cols");
            return atom_row(
                cols,
                values,
                b,
                &mut self.first_model,
                &mut self.last_model,
                &mut self.models,
                &mut self.serial,
                &mut self.subs,
            );
        }
        if let Some(t) = &mut self.table {
            self.rows += 1;
            if self.rows > 4_000_000 {
                return Err(Error::TooLarge("more than 4 million loop rows".into()));
            }
            t.rows.push(values.iter().map(|v| v.text()).collect());
        }
        Ok(())
    }

    fn loop_end(&mut self) -> Result<()> {
        if self.cols.take().is_some() {
            return Ok(());
        }
        if let Some(t) = self.table.take() {
            self.blocks.last_mut().expect("pushed").loops.push(t);
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn atom_row(
    c: &AtomCols,
    v: &[Val],
    b: &mut Build,
    first_model: &mut Option<i64>,
    last_model: &mut Option<i64>,
    models: &mut usize,
    serial: &mut i32,
    subs: &mut HashMap<Vec<u8>, u32>,
) -> Result<()> {
    let get = |i: Option<usize>| i.map(|i| v[i]).filter(|x| !x.is_null()).map(|x| x.bytes());
    if let Some(m) = get(c.model).and_then(int) {
        if *last_model != Some(m) {
            *models += 1;
            *last_model = Some(m);
        }
        match first_model {
            None => *first_model = Some(m),
            Some(f) if *f != m => return Ok(()),
            _ => {}
        }
    } else if *models == 0 {
        *models = 1;
    }
    let coord = |i: usize| {
        num(v[i].bytes())
            .ok_or_else(|| Error::Malformed("mmCIF atom_site: a coordinate is not a number".into()))
    };
    let pos = [coord(c.xyz[0])?, coord(c.xyz[1])?, coord(c.xyz[2])?].map(|x| x as f32);
    *serial += 1;
    let name = get(c.auth_atom).or(get(c.label_atom)).unwrap_or(b"");
    let comp = get(c.auth_comp).or(get(c.label_comp)).unwrap_or(b"");
    let element = get(c.symbol)
        .and_then(element::by_symbol)
        .unwrap_or_else(|| crate::pdb::element_of(b"", name, false));
    let het = get(c.group).is_some_and(|g| g.eq_ignore_ascii_case(b"HETATM"));
    let alt = get(c.alt).and_then(|a| a.first().copied()).unwrap_or(0);
    let label_asym = get(c.label_asym).unwrap_or(b"");
    let n = subs.len() as u32;
    let sub = match subs.get(label_asym) {
        Some(&s) => s,
        None => {
            subs.insert(label_asym.to_vec(), n);
            n
        }
    };
    let seq = get(c.auth_seq)
        .or(get(c.label_seq))
        .and_then(int)
        .and_then(|s| i32::try_from(s).ok())
        .unwrap_or(0);
    let icode = get(c.icode).and_then(|s| s.first().copied()).unwrap_or(0);
    let atom = Atom {
        pos,
        b: get(c.b).and_then(num).unwrap_or(0.0) as f32,
        occupancy: get(c.occ).and_then(num).unwrap_or(1.0) as f32,
        serial: get(c.id)
            .and_then(int)
            .and_then(|s| i32::try_from(s).ok())
            .unwrap_or(*serial),
        residue: 0,
        name: Tag::new(name),
        element,
        altloc: alt,
        charge: get(c.charge)
            .and_then(int)
            .and_then(|q| i8::try_from(q).ok())
            .unwrap_or(0),
        het,
    };
    b.push(AtomIn {
        chain: get(c.auth_asym).unwrap_or(label_asym),
        res_name: Tag::new(comp),
        seq,
        icode,
        sub,
        atom,
    })?;
    Ok(())
}

/// mmCIF or small-molecule CIF, from the opening bytes.
pub(crate) fn sniff(head: &[u8]) -> Option<MolFormat> {
    if head.starts_with(b"###CBF")
        || head.starts_with(b"#\\#CIF_2.0") && !contains(head, b"_atom_site")
    {
        return None;
    }
    // The first token must be a data block (after comments).
    let mut t = Tokenizer::new(head);
    match t.next_token() {
        Ok(Some(Tok::Data(_))) => {}
        _ => return None,
    }
    let lower: Vec<u8> = head.to_ascii_lowercase();
    if contains(&lower, b"_atom_site.")
        || contains(&lower, b"_entry.id")
        || contains(&lower, b"_pdbx_")
        || contains(&lower, b"_struct.")
        || contains(&lower, b"_entity.")
    {
        return Some(MolFormat::Mmcif);
    }
    if [
        &b"_cell_length"[..],
        b"_atom_site_",
        b"_symmetry_",
        b"_space_group_",
        b"_chemical_formula",
        b"_journal_",
        b"_cod_",
        b"_audit_creation",
        b"_publ_",
    ]
    .iter()
    .any(|k| contains(&lower, k))
    {
        return Some(MolFormat::Cif);
    }
    None
}

fn contains(h: &[u8], n: &[u8]) -> bool {
    h.windows(n.len()).any(|w| w == n)
}

/// Parses a CIF and builds the structure from its first block with atoms.
pub(crate) fn read(data: &[u8], limits: &Limits) -> Result<Molecule> {
    let mut c = Collector {
        blocks: Vec::new(),
        limits,
        have_atoms: false,
        table: None,
        build: None,
        cols: None,
        first_model: None,
        last_model: None,
        models: 0,
        rows: 0,
        serial: 0,
        subs: HashMap::new(),
        total_blocks: 0,
    };
    // Rough guard before tokenizing: mmCIF atom rows are 80+ bytes.
    if data.len() / 80 > limits.max_atoms.saturating_mul(4) {
        return Err(Error::TooLarge(format!("{} bytes", data.len())));
    }
    parse(data, &mut c)?;
    let block = c.blocks.pop().unwrap_or_default();
    let mut blocks = c.total_blocks.max(1);
    if c.have_atoms {
        // Blocks after the one read were not tokenized: count their headers.
        blocks = blocks.max(count_blocks(data));
    }
    if let Some(b) = c.build {
        let mut m = b.m;
        m.models = c.models.max(1);
        m.records = blocks;
        mmcif_info(&mut m, &block);
        return Ok(m);
    }
    let mut m = small(&block, limits)?;
    m.records = blocks;
    Ok(m)
}

fn count_blocks(data: &[u8]) -> usize {
    let mut n = usize::from(data.len() >= 5 && data[..5].eq_ignore_ascii_case(b"data_"));
    let mut i = 0;
    while let Some(p) = data[i..].iter().position(|&c| c == b'\n') {
        i += p + 1;
        if data.len() >= i + 5 && data[i..i + 5].eq_ignore_ascii_case(b"data_") {
            n += 1;
        }
    }
    n
}

fn mmcif_info(m: &mut Molecule, b: &Block) {
    let s = |t: &str| b.value(t).map(str::to_owned);
    m.info.id = s("_entry_id").or_else(|| (!b.name.is_empty()).then(|| b.name.clone()));
    m.info.title = s("_struct_title");
    if let Some(t) = b.table("_exptl_method", "_exptl_") {
        let ms: Vec<String> = (0..t.rows.len())
            .filter_map(|r| t.get(r, "_exptl_method").map(str::to_owned))
            .collect();
        if !ms.is_empty() {
            m.info.method = Some(ms.join(", "));
        }
    }
    m.info.resolution = b
        .number("_refine_ls_d_res_high")
        .or_else(|| b.number("_em_3d_reconstruction_resolution"))
        .or_else(|| b.number("_reflns_d_resolution_high"))
        .filter(|r| *r > 0.0);
    m.info.space_group =
        s("_symmetry_space_group_name_h-m").or_else(|| s("_space_group_name_h-m_alt"));
    m.info.classification = s("_struct_keywords_pdbx_keywords");
    m.info.date = s("_pdbx_database_status_recvd_initial_deposition_date");
    m.cell = cell_of(b);
    if let Some(t) = b.table("_citation_id", "_citation_") {
        let row = (0..t.rows.len())
            .find(|&r| t.get(r, "_citation_id") == Some("primary"))
            .unwrap_or(0);
        m.info.citation = citation(
            t.get(row, "_citation_title"),
            t.get(row, "_citation_journal_abbrev"),
            t.get(row, "_citation_year"),
            t.get(row, "_citation_pdbx_database_id_doi"),
        );
    }
    if let Some(t) = b.table("_entity_pdbx_description", "_entity_") {
        let mut v: Vec<(bool, String)> = (0..t.rows.len())
            .filter_map(|r| {
                let d = t.get(r, "_entity_pdbx_description")?;
                Some((t.get(r, "_entity_type") != Some("polymer"), d.to_owned()))
            })
            .collect();
        v.sort_by_key(|x| x.0);
        m.info.entities = v.into_iter().map(|x| x.1).collect();
    }
    // Secondary structure.
    let mut ranges: Vec<crate::ss::SsRange> = Vec::new();
    if let Some(t) = b.table("_struct_conf_conf_type_id", "_struct_conf_") {
        for r in 0..t.rows.len() {
            if !t
                .get(r, "_struct_conf_conf_type_id")
                .is_some_and(|k| k.to_ascii_uppercase().starts_with("HELX"))
            {
                continue;
            }
            if let Some(x) = range_of(&t, r, "_struct_conf_", Ss::Helix) {
                ranges.push(x);
            }
        }
    }
    if let Some(t) = b.table(
        "_struct_sheet_range_beg_auth_seq_id",
        "_struct_sheet_range_",
    ) {
        for r in 0..t.rows.len() {
            if let Some(x) = range_of(&t, r, "_struct_sheet_range_", Ss::Strand) {
                ranges.push(x);
            }
        }
    }
    if crate::ss::apply(m, &ranges) {
        m.ss_source = SsSource::File;
    }
}

fn range_of(t: &Table, r: usize, p: &str, ss: Ss) -> Option<crate::ss::SsRange> {
    let g = |k: &str| t.get(r, &format!("{p}{k}"));
    let seq = |k: &str| {
        g(k).and_then(|v| int(v.as_bytes()))
            .and_then(|v| i32::try_from(v).ok())
    };
    let ic = |k: &str| g(k).and_then(|v| v.bytes().next()).unwrap_or(0);
    Some(crate::ss::SsRange {
        chain: g("beg_auth_asym_id")?.to_owned(),
        from: (seq("beg_auth_seq_id")?, ic("pdbx_beg_pdb_ins_code")),
        to: (seq("end_auth_seq_id")?, ic("pdbx_end_pdb_ins_code")),
        ss,
    })
}

fn citation(
    title: Option<&str>,
    journal: Option<&str>,
    year: Option<&str>,
    doi: Option<&str>,
) -> Option<String> {
    let mut out = String::new();
    if let Some(t) = title {
        out.push_str(t.trim_end_matches('.'));
        out.push('.');
    }
    match (journal, year) {
        (Some(j), Some(y)) => out.push_str(&format!(" {j} ({y})")),
        (Some(j), None) => out.push_str(&format!(" {j}")),
        (None, Some(y)) => out.push_str(&format!(" ({y})")),
        _ => {}
    }
    if let Some(d) = doi {
        out.push_str(&format!(" doi:{d}"));
    }
    let out = out.trim().to_owned();
    (!out.is_empty()).then_some(out)
}

fn cell_of(b: &Block) -> Option<Cell> {
    let n = |k: &str| b.number(k);
    let c = Cell {
        a: n("_cell_length_a")?,
        b: n("_cell_length_b")?,
        c: n("_cell_length_c")?,
        alpha: n("_cell_angle_alpha").unwrap_or(90.0),
        beta: n("_cell_angle_beta").unwrap_or(90.0),
        gamma: n("_cell_angle_gamma").unwrap_or(90.0),
    };
    c.is_valid().then_some(c)
}

/// The element of a small-molecule site: its type symbol (`O2-`, `Fe3+`), else its label's
/// leading letters (`C12A`, `Cl1`).
fn site_element(symbol: Option<&str>, label: &str) -> u8 {
    for s in [symbol.unwrap_or(""), label] {
        let letters: Vec<u8> = s.bytes().take_while(u8::is_ascii_alphabetic).collect();
        if letters.len() >= 2
            && letters[1].is_ascii_lowercase()
            && let Some(e) = element::by_symbol(&letters[..2])
        {
            return e;
        }
        if s.len() == letters.len()
            && letters.len() == 2
            && let Some(e) = element::by_symbol(&letters)
        {
            return e;
        }
        if let Some(e) = letters.first().and_then(|&c| element::by_symbol(&[c])) {
            return e;
        }
    }
    0
}

/// A small-molecule CIF: fractional sites filled into the unit cell.
fn small(b: &Block, limits: &Limits) -> Result<Molecule> {
    let cell = cell_of(b).ok_or_else(|| {
        Error::Unsupported("CIF without atom_site coordinates or a unit cell".into())
    })?;
    let t = b
        .table("_atom_site_fract_x", "_atom_site_")
        .ok_or(Error::Empty)?;
    let fx = t.col("_atom_site_fract_x").ok_or(Error::Empty)?;
    let fy = t.col("_atom_site_fract_y").ok_or(Error::Empty)?;
    let fz = t.col("_atom_site_fract_z").ok_or(Error::Empty)?;
    struct Site {
        label: String,
        element: u8,
        frac: [f64; 3],
        b: f32,
        occ: f32,
    }
    let mut sites: Vec<Site> = Vec::new();
    let eight_pi2 = 8.0 * std::f64::consts::PI * std::f64::consts::PI;
    for (r, row) in t.rows.iter().enumerate() {
        let get = |c: usize| row.get(c).and_then(|v| v.as_deref());
        let f = [fx, fy, fz].map(|c| get(c).and_then(|v| num(v.as_bytes())));
        let [Some(x), Some(y), Some(z)] = f else {
            continue;
        };
        let label = t
            .get(r, "_atom_site_label")
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{}", r + 1));
        let element = site_element(t.get(r, "_atom_site_type_symbol"), &label);
        let biso = t
            .get(r, "_atom_site_u_iso_or_equiv")
            .and_then(|v| num(v.as_bytes()))
            .map(|u| u * eight_pi2)
            .or_else(|| {
                t.get(r, "_atom_site_b_iso_or_equiv")
                    .and_then(|v| num(v.as_bytes()))
            })
            .unwrap_or(0.0);
        let occ = t
            .get(r, "_atom_site_occupancy")
            .and_then(|v| num(v.as_bytes()))
            .unwrap_or(1.0);
        sites.push(Site {
            label,
            element,
            frac: [x, y, z],
            b: biso as f32,
            occ: occ as f32,
        });
        if sites.len() > limits.max_atoms {
            return Err(Error::TooLarge(format!(
                "more than {} sites",
                limits.max_atoms
            )));
        }
    }
    if sites.is_empty() {
        return Err(Error::Empty);
    }
    // Symmetry operators.
    let mut ops: Vec<SymOp> = Vec::new();
    let mut notes = Vec::new();
    for tag in [
        "_space_group_symop_operation_xyz",
        "_symmetry_equiv_pos_as_xyz",
    ] {
        if let Some(t) = b.table(tag, tag) {
            for r in 0..t.rows.len() {
                if let Some(s) = t.get(r, tag) {
                    match SymOp::parse(s) {
                        Some(op) => ops.push(op),
                        None => notes.push(format!("unreadable symmetry operator {s:?}")),
                    }
                }
            }
            if !ops.is_empty() {
                break;
            }
        }
        if ops.len() > 192 {
            return Err(Error::Malformed(format!(
                "{} symmetry operators",
                ops.len()
            )));
        }
    }
    if ops.is_empty() {
        notes.push("no symmetry operators: the asymmetric unit only".into());
        ops.push(SymOp::IDENTITY);
    }
    ops.truncate(192);
    // Bonds in the asymmetric unit: from _geom_bond (those without a symmetry code), else
    // by distance.
    let index: HashMap<&str, usize> = sites
        .iter()
        .enumerate()
        .map(|(i, s)| (s.label.as_str(), i))
        .collect();
    let mut asym_bonds: Vec<(usize, usize)> = Vec::new();
    // Bonds to symmetry images: label pairs and their distances.
    let mut cross: Vec<(usize, usize, f64)> = Vec::new();
    let mut file_bonds = 0;
    if let Some(gt) = b.table("_geom_bond_atom_site_label_1", "_geom_bond_") {
        for r in 0..gt.rows.len() {
            let (Some(a), Some(c)) = (
                gt.get(r, "_geom_bond_atom_site_label_1"),
                gt.get(r, "_geom_bond_atom_site_label_2"),
            ) else {
                continue;
            };
            let (Some(&i), Some(&j)) = (index.get(a), index.get(c)) else {
                continue;
            };
            file_bonds += 1;
            let sym = gt
                .get(r, "_geom_bond_site_symmetry_2")
                .filter(|s| *s != "1_555" && *s != ".");
            match sym {
                None => asym_bonds.push((i.min(j), i.max(j))),
                Some(_) => {
                    let d = gt
                        .get(r, "_geom_bond_distance")
                        .and_then(|v| num(v.as_bytes()))
                        .unwrap_or(0.0);
                    cross.push((i, j, d));
                }
            }
        }
    }
    let cart: Vec<[f32; 3]> = sites
        .iter()
        .map(|s| cell.to_cartesian(s.frac).map(|v| v as f32))
        .collect();
    if file_bonds == 0 {
        let els: Vec<u8> = sites.iter().map(|s| s.element).collect();
        for bd in crate::bonds::bond_by_distance(&cart, &els, None) {
            asym_bonds.push((bd.0 as usize, bd.1 as usize));
        }
    }
    asym_bonds.sort_unstable();
    asym_bonds.dedup();
    // Connected components of the asymmetric unit.
    let mut parent: Vec<usize> = (0..sites.len()).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for &(i, j) in &asym_bonds {
        let (a, c) = (root(&mut parent, i), root(&mut parent, j));
        if a != c {
            parent[a] = c;
        }
    }
    let mut comps: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..sites.len() {
        let r = root(&mut parent, i);
        comps.entry(r).or_default().push(i);
    }
    let mut comps: Vec<Vec<usize>> = comps.into_values().collect();
    comps.sort();
    if sites.len().saturating_mul(ops.len()) > limits.max_cell_atoms {
        return Err(Error::TooLarge(format!(
            "{} sites × {} symmetry operators",
            sites.len(),
            ops.len()
        )));
    }
    // Fill the cell: each component's image is moved whole so its centroid lies in the
    // cell; sites that land on one already placed (special positions) are dropped.
    let mut grid = PeriodicGrid::new(&cell);
    let mut build = Build::new(MolFormat::Cif, limits);
    // (image atom index, asym site) for the bonds below.
    let mut placed: Vec<(usize, usize)> = Vec::new();
    let mut bonds: Vec<Bond> = Vec::new();
    for op in &ops {
        for comp in &comps {
            let fr: Vec<[f64; 3]> = comp.iter().map(|&i| op.apply(sites[i].frac)).collect();
            let n = fr.len() as f64;
            let cen = [0, 1, 2].map(|k| fr.iter().map(|p| p[k]).sum::<f64>() / n);
            let shift = cen.map(|c| -(c + 1e-6).floor());
            let mut local: HashMap<usize, u32> = HashMap::new();
            for (&i, f) in comp.iter().zip(&fr) {
                let f = [f[0] + shift[0], f[1] + shift[1], f[2] + shift[2]];
                if grid.occupied(f) {
                    continue;
                }
                grid.insert(f);
                let s = &sites[i];
                let p = cell.to_cartesian(f);
                let at = build.m.atoms.len() as u32;
                build.push(AtomIn {
                    chain: b"",
                    res_name: Tag::new(b"UNL"),
                    seq: 1,
                    icode: 0,
                    sub: 0,
                    atom: Atom {
                        pos: p.map(|v| v as f32),
                        b: s.b,
                        occupancy: s.occ,
                        serial: at as i32 + 1,
                        residue: 0,
                        name: Tag::new(s.label.as_bytes()),
                        element: s.element,
                        altloc: 0,
                        charge: 0,
                        het: true,
                    },
                })?;
                local.insert(i, at);
                placed.push((at as usize, i));
            }
            for &(i, j) in &asym_bonds {
                if let (Some(&a), Some(&c)) = (local.get(&i), local.get(&j)) {
                    bonds.push(Bond {
                        a: a.min(c),
                        b: a.max(c),
                        order: 0,
                    });
                }
            }
        }
    }
    // Bonds to symmetry images: the listed label pairs at the listed distance.
    if !cross.is_empty() {
        let pos: Vec<[f32; 3]> = build.m.atoms.iter().map(|a| a.pos).collect();
        let els: Vec<u8> = build.m.atoms.iter().map(|a| a.element).collect();
        let site_of: Vec<usize> = {
            let mut v = vec![0; pos.len()];
            for &(a, s) in &placed {
                v[a] = s;
            }
            v
        };
        for (a, c) in crate::bonds::bond_by_distance(&pos, &els, Some(0.6)) {
            let (sa, sc) = (site_of[a as usize], site_of[c as usize]);
            let d = f64::from(dist(pos[a as usize], pos[c as usize]));
            if cross.iter().any(|&(i, j, dd)| {
                ((i == sa && j == sc) || (i == sc && j == sa))
                    && (dd == 0.0 || (d - dd).abs() < 0.1)
            }) {
                bonds.push(Bond { a, b: c, order: 0 });
            }
        }
    }
    bonds.sort_unstable_by_key(|b| (b.a, b.b));
    bonds.dedup_by_key(|b| (b.a, b.b));
    let mut m = build.m;
    m.bonds = bonds;
    m.file_bonds = file_bonds;
    m.cell = Some(cell);
    m.symops = ops;
    m.asymmetric_atoms = sites.len();
    m.notes = notes;
    let s = |t: &str| b.value(t).map(str::to_owned);
    m.info.id = s("_cod_database_code").or_else(|| (!b.name.is_empty()).then(|| b.name.clone()));
    m.info.title = s("_chemical_name_common")
        .or_else(|| s("_chemical_name_systematic"))
        .or_else(|| s("_publ_section_title"))
        .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "));
    m.info.formula = s("_chemical_formula_sum");
    m.info.space_group =
        s("_space_group_name_h-m_alt").or_else(|| s("_symmetry_space_group_name_h-m"));
    m.info.method = s("_diffrn_radiation_probe")
        .or_else(|| s("_exptl_method"))
        .or_else(|| {
            b.items
                .keys()
                .any(|k| k.starts_with("_diffrn_"))
                .then(|| "X-ray diffraction".into())
        });
    m.info.citation = citation(
        s("_publ_section_title")
            .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
            .as_deref(),
        s("_journal_name_full").as_deref(),
        s("_journal_year").as_deref(),
        s("_journal_paper_doi").as_deref(),
    );
    m.ss_source = SsSource::None;
    Ok(m)
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// Sites in a cell, found modulo lattice translations.
struct PeriodicGrid {
    cell: Cell,
    n: [usize; 3],
    bins: HashMap<[usize; 3], Vec<[f64; 3]>>,
}

/// Two sites closer than this (Å) are one.
const SAME_SITE: f64 = 0.35;

impl PeriodicGrid {
    fn new(cell: &Cell) -> Self {
        let n = [cell.a, cell.b, cell.c].map(|l| ((l / 1.0).floor() as usize).clamp(1, 256));
        PeriodicGrid {
            cell: *cell,
            n,
            bins: HashMap::new(),
        }
    }

    fn wrap(f: [f64; 3]) -> [f64; 3] {
        f.map(|v| {
            let w = v - v.floor();
            if w >= 1.0 { 0.0 } else { w }
        })
    }

    fn bin(&self, w: [f64; 3]) -> [usize; 3] {
        [0, 1, 2].map(|k| ((w[k] * self.n[k] as f64) as usize).min(self.n[k] - 1))
    }

    fn occupied(&self, f: [f64; 3]) -> bool {
        let w = Self::wrap(f);
        let b = self.bin(w);
        for dx in -1i64..=1 {
            for dy in -1i64..=1 {
                for dz in -1i64..=1 {
                    let k = [0, 1, 2].map(|i| {
                        let d = [dx, dy, dz][i];
                        let n = self.n[i] as i64;
                        ((b[i] as i64 + d).rem_euclid(n)) as usize
                    });
                    if let Some(v) = self.bins.get(&k) {
                        for o in v {
                            // Minimum image.
                            let d = [0, 1, 2].map(|i| {
                                let x = w[i] - o[i];
                                x - x.round()
                            });
                            let c = self.cell.to_cartesian(d);
                            if (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt() < SAME_SITE {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    fn insert(&mut self, f: [f64; 3]) {
        let w = Self::wrap(f);
        let b = self.bin(w);
        self.bins.entry(b).or_default().push(w);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct All(Vec<String>);

    impl<'a> Sink<'a> for All {
        fn block(&mut self, name: &'a [u8]) -> Result<bool> {
            self.0
                .push(format!("data {}", String::from_utf8_lossy(name)));
            Ok(true)
        }
        fn item(&mut self, tag: &'a [u8], v: Val<'a>) -> Result<()> {
            self.0.push(format!(
                "{} = {:?}",
                String::from_utf8_lossy(tag),
                String::from_utf8_lossy(v.bytes())
            ));
            Ok(())
        }
        fn loop_start(&mut self, tags: &[&'a [u8]]) -> Result<()> {
            self.0.push(format!("loop {}", tags.len()));
            Ok(())
        }
        fn row(&mut self, values: &[Val<'a>]) -> Result<()> {
            let v: Vec<String> = values
                .iter()
                .map(|v| String::from_utf8_lossy(v.bytes()).into_owned())
                .collect();
            self.0.push(format!("row {}", v.join("|")));
            Ok(())
        }
        fn loop_end(&mut self) -> Result<()> {
            self.0.push("end".into());
            Ok(())
        }
    }

    fn tokens(s: &str) -> Result<Vec<String>> {
        let mut a = All(Vec::new());
        parse(s.as_bytes(), &mut a)?;
        Ok(a.0)
    }

    #[test]
    fn tokenizer_cases() {
        let t = tokens(
            "# comment\ndata_x\n_a.b 'it''s here'\n_c \"q\" #c\n_d\n;line one\nline two\n;\nloop_\n_l.x _l.y\n1 'a b' . ?\n;t\n;\nz\nstop_ global_\n",
        )
        .unwrap();
        assert_eq!(
            t,
            [
                "data x",
                "_a.b = \"it''s here\"",
                "_c = \"q\"",
                "_d = \"line one\\nline two\"",
                "loop 2",
                "row 1|a b",
                "row .|?",
                "row t|z",
            ]
            .iter()
            .map(|s| s.to_string())
            .chain(["end".to_string()])
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn quotes_close_only_before_whitespace() {
        let t = tokens("data_q\n_a 'O'Brien's'\n_b x'y\n").unwrap();
        assert_eq!(t[1], "_a = \"O'Brien's\"");
        assert_eq!(t[2], "_b = \"x'y\"");
    }

    #[test]
    fn triple_quotes() {
        let t = tokens("#\\#CIF_2.0\ndata_t\n_a '''multi\nline'''\n").unwrap();
        assert_eq!(t[1], "_a = \"multi\\nline\"");
    }

    #[test]
    fn refusals() {
        for (bad, why) in [
            ("data_x\n_a 'open\n", "past the end of the line"),
            ("data_x\n_a\n;never closed\n", "unterminated text field"),
            ("data_x\nloop_\n_a _b\n1 2 3\n", "whole number of rows"),
            ("data_x\n_a\n_b 1\n", "a tag without a value"),
            ("data_x\nvalue\n", "a value without a tag"),
            ("data_x\nloop_\n1\n", "loop_ without tags"),
        ] {
            let e = tokens(bad).unwrap_err().to_string();
            assert!(e.contains(why), "{bad:?}: {e}");
        }
    }

    #[test]
    fn crlf_text_fields() {
        let t = tokens("data_c\r\n_a\r\n;one\r\ntwo\r\n;\r\n_b 2\r\n").unwrap();
        assert_eq!(t[1], "_a = \"one\\r\\ntwo\"");
        assert_eq!(t[2], "_b = \"2\"");
    }

    #[test]
    fn site_elements() {
        assert_eq!(site_element(Some("O2-"), "O1"), 8);
        assert_eq!(site_element(Some("Fe3+"), "Fe1"), 26);
        assert_eq!(site_element(None, "Cl1"), 17);
        assert_eq!(site_element(None, "C12A"), 6);
        assert_eq!(site_element(None, "H1A"), 1);
        assert_eq!(site_element(Some("CU"), "Cu1"), 29);
    }

    #[test]
    fn sniffing() {
        assert_eq!(
            sniff(b"data_1ABC\n#\n_entry.id 1ABC\n"),
            Some(MolFormat::Mmcif)
        );
        assert_eq!(
            sniff(b"#COD\ndata_1000\n_cell_length_a 5.0\n"),
            Some(MolFormat::Cif)
        );
        assert_eq!(sniff(b"###CBF: VERSION 1.5\ndata_image\n"), None);
        assert_eq!(sniff(b"hello\n"), None);
        assert_eq!(sniff(b"data_x\n_array_data.data ?\n"), None);
    }
}
