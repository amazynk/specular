//! What each fabrication file of a board is: from its X2 `.FileFunction`, else its name.

/// A side of the board.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Top,
    Bottom,
}

/// Whether a drill file's holes are plated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Plating {
    Plated,
    NonPlated,
    /// Both, or not said.
    Mixed,
}

/// A file's role in the board's stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    TopCopper,
    BottomCopper,
    /// Inner copper layer `n`, counted from the top from 1.
    InnerCopper(u32),
    SolderMask(Side),
    Silkscreen(Side),
    Paste(Side),
    /// The board outline (profile, edge cuts, keep-out of the board).
    Outline,
    Drill(Plating),
    /// Mechanical layers, fabrication and assembly drawings.
    Mechanical,
    /// Known to be something else (glue, carbon, V-cut, component files).
    Other,
    Unknown,
}

impl Role {
    /// The English name ("Top copper").
    pub fn english(self) -> String {
        match self {
            Role::TopCopper => "Top copper".into(),
            Role::BottomCopper => "Bottom copper".into(),
            Role::InnerCopper(n) => format!("Inner copper {n}"),
            Role::SolderMask(Side::Top) => "Top solder mask".into(),
            Role::SolderMask(Side::Bottom) => "Bottom solder mask".into(),
            Role::Silkscreen(Side::Top) => "Top silkscreen".into(),
            Role::Silkscreen(Side::Bottom) => "Bottom silkscreen".into(),
            Role::Paste(Side::Top) => "Top paste".into(),
            Role::Paste(Side::Bottom) => "Bottom paste".into(),
            Role::Outline => "Board outline".into(),
            Role::Drill(Plating::Plated) => "Plated drill".into(),
            Role::Drill(Plating::NonPlated) => "Non-plated drill".into(),
            Role::Drill(Plating::Mixed) => "Drill".into(),
            Role::Mechanical => "Mechanical".into(),
            Role::Other => "Other".into(),
            Role::Unknown => "Unknown".into(),
        }
    }

    pub fn is_copper(self) -> bool {
        matches!(
            self,
            Role::TopCopper | Role::BottomCopper | Role::InnerCopper(_)
        )
    }

    /// Where it goes in the stack, drawn from the lowest: bottom side up to the top side,
    /// then the outline and the drills over everything.
    pub fn rank(self) -> u32 {
        match self {
            Role::Silkscreen(Side::Bottom) => 0,
            Role::Paste(Side::Bottom) => 1,
            Role::SolderMask(Side::Bottom) => 2,
            Role::BottomCopper => 3,
            Role::InnerCopper(n) => 200 - n.min(150),
            Role::TopCopper => 210,
            Role::SolderMask(Side::Top) => 211,
            Role::Paste(Side::Top) => 212,
            Role::Silkscreen(Side::Top) => 213,
            Role::Mechanical => 220,
            Role::Other => 221,
            Role::Unknown => 222,
            Role::Outline => 230,
            Role::Drill(_) => 240,
        }
    }

    /// Its colour in the layer view: KiCad's defaults where they read on dark and light
    /// paper alike.
    pub fn colour(self) -> [u8; 3] {
        const INNER: [[u8; 3]; 6] = [
            [194, 194, 0],
            [194, 0, 194],
            [0, 150, 150],
            [150, 90, 0],
            [0, 150, 0],
            [120, 120, 220],
        ];
        match self {
            Role::TopCopper => [200, 52, 52],
            Role::BottomCopper => [77, 127, 196],
            Role::InnerCopper(n) => INNER[(n.max(1) as usize - 1) % INNER.len()],
            Role::SolderMask(Side::Top) => [190, 90, 230],
            Role::SolderMask(Side::Bottom) => [0, 160, 150],
            Role::Silkscreen(Side::Top) => [205, 170, 30],
            Role::Silkscreen(Side::Bottom) => [190, 120, 170],
            Role::Paste(Side::Top) => [170, 140, 130],
            Role::Paste(Side::Bottom) => [0, 170, 190],
            Role::Outline => [210, 190, 40],
            Role::Drill(_) => [130, 130, 130],
            Role::Mechanical => [140, 120, 90],
            Role::Other | Role::Unknown => [150, 150, 150],
        }
    }

    /// Whether a file with this role is a drill file.
    pub fn is_drill(self) -> bool {
        matches!(self, Role::Drill(_))
    }
}

/// The role X2 `.FileFunction` values (`Copper,L1,Top`) give, if any.
pub fn role_of_x2(function: &str) -> Option<Role> {
    let v: Vec<&str> = function.split(',').map(str::trim).collect();
    let side = |i: usize| match v.get(i).map(|s| s.to_ascii_lowercase()) {
        Some(s) if s.starts_with("bot") => Side::Bottom,
        _ => Side::Top,
    };
    let first = v.first()?.to_ascii_lowercase();
    Some(match first.as_str() {
        "copper" => {
            let n: u32 = v
                .get(1)
                .and_then(|l| l.trim_start_matches(['L', 'l']).parse().ok())
                .unwrap_or(1);
            match v.get(2).map(|s| s.to_ascii_lowercase()) {
                Some(s) if s.starts_with("top") => Role::TopCopper,
                Some(s) if s.starts_with("bot") => Role::BottomCopper,
                _ if n <= 1 => Role::TopCopper,
                _ => Role::InnerCopper(n - 1),
            }
        }
        "soldermask" => Role::SolderMask(side(1)),
        "legend" => Role::Silkscreen(side(1)),
        "paste" => Role::Paste(side(1)),
        "profile" => Role::Outline,
        "plated" => Role::Drill(Plating::Plated),
        "nonplated" => Role::Drill(Plating::NonPlated),
        "mixedplating" => Role::Drill(Plating::Mixed),
        "drillmap" | "fabricationdrawing" | "assemblydrawing" | "arrangementdrawing"
        | "otherdrawing" | "keep-out" | "drawing" => Role::Mechanical,
        "" => return None,
        _ => Role::Other,
    })
}

/// A file's role from X2 `.FileFunction` first, then its name: Protel/Altium extensions,
/// KiCad, Eagle and generic names.
pub fn role_of(file_name: &str, x2_file_function: Option<&str>) -> Role {
    if let Some(r) = x2_file_function.and_then(role_of_x2) {
        return r;
    }
    role_by_name(file_name)
}

/// A file's role from its name alone.
pub fn role_by_name(file_name: &str) -> Role {
    let base = file_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(file_name)
        .to_ascii_lowercase();
    let (stem, ext) = match base.rsplit_once('.') {
        Some((s, e)) => (s, e),
        None => (base.as_str(), ""),
    };
    // Protel / Altium extensions.
    let by_ext = match ext {
        "gtl" => Some(Role::TopCopper),
        "gbl" => Some(Role::BottomCopper),
        "gto" => Some(Role::Silkscreen(Side::Top)),
        "gbo" => Some(Role::Silkscreen(Side::Bottom)),
        "gts" => Some(Role::SolderMask(Side::Top)),
        "gbs" => Some(Role::SolderMask(Side::Bottom)),
        "gtp" => Some(Role::Paste(Side::Top)),
        "gbp" => Some(Role::Paste(Side::Bottom)),
        "gko" | "gm1" | "gml" | "gmb" | "oln" | "dim" => Some(Role::Outline),
        // Eagle's CAM job.
        "cmp" => Some(Role::TopCopper),
        "sol" => Some(Role::BottomCopper),
        "stc" => Some(Role::SolderMask(Side::Top)),
        "sts" => Some(Role::SolderMask(Side::Bottom)),
        "plc" => Some(Role::Silkscreen(Side::Top)),
        "pls" => Some(Role::Silkscreen(Side::Bottom)),
        "crc" => Some(Role::Paste(Side::Top)),
        "crs" => Some(Role::Paste(Side::Bottom)),
        "drd" | "drl" | "xln" | "exc" | "drill" | "nc" | "tap" => Some(drill_from(stem)),
        "gd1" | "gg1" | "gpt" | "gpb" | "gdd" => Some(Role::Mechanical),
        _ => None,
    };
    if let Some(r) = by_ext {
        return r;
    }
    if let Some(n) = ext.strip_prefix("gm")
        && n.parse::<u32>().is_ok()
    {
        return Role::Mechanical;
    }
    // Altium mid layers (G1…) and internal planes (GP1…), Eagle inner layers (ly2…).
    for prefix in ["gp", "g", "ly"] {
        if let Some(n) = ext.strip_prefix(prefix)
            && let Ok(n) = n.parse::<u32>()
            && (1..=64).contains(&n)
        {
            return Role::InnerCopper(if prefix == "ly" {
                n.saturating_sub(1).max(1)
            } else {
                n
            });
        }
    }
    // KiCad: "<board>-F_Cu.gbr", "<board>-In1_Cu.g2", "<board>-NPTH.drl".
    let words: Vec<&str> = stem
        .split(['-', '_', '.', ' '])
        .filter(|w| !w.is_empty())
        .collect();
    let joined = stem.replace(['-', '.', ' '], "_");
    let has = |w: &str| words.contains(&w);
    let kicad = |s: &str| joined.ends_with(s);
    if kicad("f_cu") {
        return Role::TopCopper;
    }
    if kicad("b_cu") {
        return Role::BottomCopper;
    }
    if let Some(w) = words
        .iter()
        .rev()
        .find(|w| w.starts_with("in") && w.len() > 2)
        && let Ok(n) = w[2..].parse::<u32>()
        && has("cu")
    {
        return Role::InnerCopper(n);
    }
    if kicad("f_mask") {
        return Role::SolderMask(Side::Top);
    }
    if kicad("b_mask") {
        return Role::SolderMask(Side::Bottom);
    }
    if kicad("f_silkscreen") || kicad("f_silks") {
        return Role::Silkscreen(Side::Top);
    }
    if kicad("b_silkscreen") || kicad("b_silks") {
        return Role::Silkscreen(Side::Bottom);
    }
    if kicad("f_paste") {
        return Role::Paste(Side::Top);
    }
    if kicad("b_paste") {
        return Role::Paste(Side::Bottom);
    }
    if kicad("edge_cuts") {
        return Role::Outline;
    }
    if has("npth") {
        return Role::Drill(Plating::NonPlated);
    }
    if has("pth") {
        return Role::Drill(Plating::Plated);
    }
    if kicad("f_fab")
        || kicad("b_fab")
        || kicad("f_courtyard")
        || kicad("b_courtyard")
        || kicad("dwgs_user")
        || kicad("cmts_user")
        || kicad("user_drawings")
        || kicad("margin")
        || kicad("eco1_user")
        || kicad("eco2_user")
    {
        return Role::Mechanical;
    }
    // Generic names.
    if stem.contains("inner") || has("inr") || has("mid") || stem.contains("internal") {
        let num = |w: &str| -> Option<(u32, bool)> {
            if let Some(n) = w.strip_prefix('l').and_then(|n| n.parse::<u32>().ok()) {
                return Some((n, true));
            }
            for p in ["layer", "inner", "in", "mid", "inr"] {
                if let Some(n) = w.strip_prefix(p).and_then(|n| n.parse::<u32>().ok()) {
                    return Some((n, false));
                }
            }
            w.parse::<u32>().ok().map(|n| (n, false))
        };
        let n = words.iter().rev().find_map(|w| num(w));
        return Role::InnerCopper(match n {
            // "L2" counts the top layer as 1.
            Some((n, true)) => n.saturating_sub(1).max(1),
            Some((n, false)) => n.max(1),
            None => 1,
        });
    }
    let top = has("top") || has("front") || stem.contains("top");
    let bottom = has("bottom") || has("bot") || has("back") || stem.contains("bottom");
    let side = if bottom && !top {
        Side::Bottom
    } else {
        Side::Top
    };
    if stem.contains("mask") || has("sm") || has("smt") || has("smb") {
        return Role::SolderMask(side);
    }
    if stem.contains("silk") || stem.contains("legend") || stem.contains("overlay") {
        return Role::Silkscreen(side);
    }
    if stem.contains("paste") || stem.contains("cream") || stem.contains("stencil") {
        return Role::Paste(side);
    }
    if stem.contains("outline")
        || stem.contains("edge")
        || stem.contains("profile")
        || stem.contains("contour")
        || stem.contains("milling")
        || has("dim")
    {
        return Role::Outline;
    }
    if stem.contains("drill") || has("holes") || stem.contains("hole") || has("drl") {
        return drill_from(stem);
    }
    if ext == "txt" {
        return Role::Unknown;
    }
    if stem.contains("copper")
        || stem.contains("signal")
        || stem.contains("layer")
        || has("cu")
        || top
        || bottom
    {
        return if bottom && !top {
            Role::BottomCopper
        } else {
            Role::TopCopper
        };
    }
    Role::Unknown
}

fn drill_from(stem: &str) -> Role {
    let s = stem.to_ascii_lowercase();
    if s.contains("npth") || s.contains("non") || s.contains("unplated") {
        Role::Drill(Plating::NonPlated)
    } else if s.contains("pth") || s.contains("plated") {
        Role::Drill(Plating::Plated)
    } else {
        Role::Drill(Plating::Mixed)
    }
}

/// File names that a board's fabrication set may hold besides its layers.
fn is_extra(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    let ext = n.rsplit_once('.').map_or("", |(_, e)| e);
    matches!(
        ext,
        "txt"
            | "md"
            | "pdf"
            | "rep"
            | "rpt"
            | "gbrjob"
            | "job"
            | "ini"
            | "csv"
            | "pos"
            | "xyz"
            | "html"
            | "htm"
            | "json"
            | "apr"
            | "lst"
            | "log"
            | "extrep"
            | "drr"
            | "cam"
            | "bom"
            | "readme"
            | "ldp"
            | "ipc"
            | "zip"
    ) || n.contains("readme")
        || n.starts_with('.')
}

/// Whether a folder or archive listing (file names) is a board's fabrication set: at least
/// two layer files known by name, one of them copper or the outline, and little else.
pub fn looks_like_board(names: &[&str]) -> bool {
    let mut known = 0;
    let mut anchor = false;
    let mut other = 0;
    for n in names {
        let base = n.rsplit(['/', '\\']).next().unwrap_or(n);
        if base.is_empty() || is_extra(base) {
            continue;
        }
        let r = role_by_name(base);
        let gerberish = {
            let l = base.to_ascii_lowercase();
            l.ends_with(".gbr")
                || l.ends_with(".ger")
                || l.ends_with(".pho")
                || l.ends_with(".art")
                || l.ends_with(".gdo")
                || l.ends_with(".gbx")
        };
        match r {
            Role::Unknown | Role::Other if !gerberish => {
                if !is_extra(base) {
                    other += 1;
                }
            }
            Role::Unknown | Role::Other => known += 1,
            r => {
                known += 1;
                if r.is_copper() || r == Role::Outline {
                    anchor = true;
                }
            }
        }
    }
    known >= 2 && anchor && other <= known
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x2_functions() {
        assert_eq!(role_of("x.gbr", Some("Copper,L1,Top")), Role::TopCopper);
        assert_eq!(
            role_of("x.gbr", Some("Copper,L4,Bot,Signal")),
            Role::BottomCopper
        );
        assert_eq!(
            role_of("x.gbr", Some("Copper,L2,Inr,Plane")),
            Role::InnerCopper(1)
        );
        assert_eq!(
            role_of("x.gbr", Some("Soldermask,Bot")),
            Role::SolderMask(Side::Bottom)
        );
        assert_eq!(
            role_of("x.gbr", Some("Legend,Top")),
            Role::Silkscreen(Side::Top)
        );
        assert_eq!(role_of("x.gbr", Some("Profile,NP")), Role::Outline);
        assert_eq!(
            role_of("x.drl", Some("NonPlated,1,2,NPTH")),
            Role::Drill(Plating::NonPlated)
        );
        // X2 wins over the name.
        assert_eq!(
            role_of("top.gbl", Some("Paste,Top")),
            Role::Paste(Side::Top)
        );
    }

    #[test]
    fn names() {
        let cases = [
            ("board.GTL", Role::TopCopper),
            ("board.GBL", Role::BottomCopper),
            ("board.GTO", Role::Silkscreen(Side::Top)),
            ("board.GBS", Role::SolderMask(Side::Bottom)),
            ("board.GKO", Role::Outline),
            ("board.G2", Role::InnerCopper(2)),
            ("board.GP1", Role::InnerCopper(1)),
            ("board.GM13", Role::Mechanical),
            ("board.XLN", Role::Drill(Plating::Mixed)),
            ("demo-F_Cu.gbr", Role::TopCopper),
            ("demo-B_Cu.gbr", Role::BottomCopper),
            ("demo-In2_Cu.gbr", Role::InnerCopper(2)),
            ("demo-F_Mask.gbr", Role::SolderMask(Side::Top)),
            ("demo-B_Silkscreen.gbr", Role::Silkscreen(Side::Bottom)),
            ("demo-F_SilkS.gbr", Role::Silkscreen(Side::Top)),
            ("demo-F_Paste.gbr", Role::Paste(Side::Top)),
            ("demo-Edge_Cuts.gbr", Role::Outline),
            ("demo-NPTH.drl", Role::Drill(Plating::NonPlated)),
            ("demo-PTH.drl", Role::Drill(Plating::Plated)),
            ("demo-F_Fab.gbr", Role::Mechanical),
            ("board.cmp", Role::TopCopper),
            ("board.sol", Role::BottomCopper),
            ("board.stc", Role::SolderMask(Side::Top)),
            ("board.plc", Role::Silkscreen(Side::Top)),
            ("board.dim", Role::Outline),
            ("board.drd", Role::Drill(Plating::Mixed)),
            ("TopLayer.gbr", Role::TopCopper),
            ("bottom_soldermask.gbr", Role::SolderMask(Side::Bottom)),
            ("silkscreen_top.ger", Role::Silkscreen(Side::Top)),
            ("BoardOutline.gbr", Role::Outline),
            ("README.txt", Role::Unknown),
            ("drill_plated.txt", Role::Drill(Plating::Plated)),
            ("copper_inner_l2.gbr", Role::InnerCopper(1)),
            ("copper_top_l1.gbr", Role::TopCopper),
            ("board_inner3.gbr", Role::InnerCopper(3)),
        ];
        for (n, want) in cases {
            assert_eq!(role_by_name(n), want, "{n}");
        }
    }

    #[test]
    fn board_listings() {
        assert!(looks_like_board(&[
            "a/demo-F_Cu.gbr",
            "a/demo-B_Cu.gbr",
            "a/demo-Edge_Cuts.gbr",
            "a/demo-job.gbrjob",
            "a/demo.drl",
            "a/readme.txt",
        ]));
        assert!(looks_like_board(&["b.GTL", "b.GBL", "b.TXT"]));
        assert!(!looks_like_board(&["photo.jpg", "b.GTL"]));
        assert!(!looks_like_board(&["notes.txt", "x.pdf"]));
        assert!(!looks_like_board(&[
            "TOPSIDE.pdf",
            "BOTTOMSIDE.pdf",
            "README"
        ]));
        assert!(!looks_like_board(&[
            "b.GTL", "1.jpg", "2.jpg", "3.jpg", "4.jpg", "b.GTO"
        ]));
    }
}
