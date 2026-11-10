//! Batch rename: the preview of what each selected file would be called, with problems
//! marked, and the rename itself (sidecars following, swaps through temporary names).
//!
//! The names come from the caller (the app renders `{name}`, `{n:3}`, `{date:…}`, `{camera}`
//! with specular-convert's pattern); this module checks and applies them.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use crate::history::{Kind, Op};
use crate::names::{self, NameProblem, Platform};

/// One row of the preview.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub from: PathBuf,
    pub new_name: String,
    /// Why this row can't be renamed; `None` when it can (or when the name is unchanged).
    pub problem: Option<RowProblem>,
}

impl Row {
    pub fn unchanged(&self) -> bool {
        self.from
            .file_name()
            .is_some_and(|n| n.to_string_lossy() == self.new_name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowProblem {
    Name(NameProblem),
    /// Two files in the batch would get this name.
    Duplicate,
}

impl RowProblem {
    pub fn message_id(&self) -> &'static str {
        match self {
            RowProblem::Name(p) => p.message_id(),
            RowProblem::Duplicate => "rename-problem-duplicate",
        }
    }
}

/// Check every proposed name. Names are compared case-insensitively for duplicates on
/// Windows and macOS (whose disks usually ignore case).
pub fn preview(names_in: &[(PathBuf, String)], platform: Platform) -> Vec<Row> {
    let fold = |s: &str| {
        if platform == Platform::Unix {
            s.to_owned()
        } else {
            s.to_lowercase()
        }
    };
    let mut count: HashMap<(PathBuf, String), usize> = HashMap::new();
    for (from, n) in names_in {
        let dir = from.parent().unwrap_or(Path::new(".")).to_path_buf();
        *count.entry((dir, fold(n))).or_default() += 1;
    }
    let sources: Vec<&Path> = names_in.iter().map(|(f, _)| f.as_path()).collect();
    let mut writable: HashMap<PathBuf, bool> = HashMap::new();
    names_in
        .iter()
        .map(|(from, n)| {
            let dir = from.parent().unwrap_or(Path::new(".")).to_path_buf();
            let mut problem = names::check_text(n, platform).err().map(RowProblem::Name);
            if problem.is_none() && count[&(dir.clone(), fold(n))] > 1 {
                problem = Some(RowProblem::Duplicate);
            }
            let unchanged = from.file_name().is_some_and(|x| x.to_string_lossy() == *n);
            if problem.is_none() && !unchanged {
                let to = dir.join(n);
                // Taken by a file outside the batch (one inside moves away first).
                if crate::atomic::taken(&to)
                    && !sources.contains(&to.as_path())
                    && !crate::atomic::same_file(from, &to)
                {
                    problem = Some(RowProblem::Name(NameProblem::Exists));
                } else if !*writable
                    .entry(dir.clone())
                    .or_insert_with(|| crate::atomic::dir_writable(&dir))
                {
                    problem = Some(RowProblem::Name(NameProblem::ReadOnlyFolder));
                }
            }
            Row {
                from: from.clone(),
                new_name: n.clone(),
                problem,
            }
        })
        .collect()
}

/// Rename the rows that can be (problem rows and unchanged names are left alone), with each
/// file's sidecars, and the other half of a RAW+JPEG pair when that half isn't in the batch.
pub fn apply(rows: &[Row]) -> Result<Op, (PathBuf, io::Error)> {
    let selected: Vec<&Path> = rows.iter().map(|r| r.from.as_path()).collect();
    let mut moves: Vec<(PathBuf, PathBuf)> = Vec::new();
    for r in rows {
        if r.problem.is_some() || r.unchanged() {
            continue;
        }
        let dir = r.from.parent().unwrap_or(Path::new("."));
        let to = dir.join(&r.new_name);
        moves.push((r.from.clone(), to.clone()));
        for sc in names::sidecars(&r.from, true) {
            if selected.contains(&sc.as_path()) {
                continue;
            }
            if let Some(n) = names::sidecar_new_name(&sc, &r.from, &to) {
                moves.push((sc.clone(), dir.join(n)));
            }
        }
    }
    crate::transfer::rename_all(&moves, Kind::BatchRename)
}
