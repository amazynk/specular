#![allow(dead_code)]

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// A private directory under the temp dir (the lane's tmp/), removed on drop.
pub struct Dir(pub PathBuf);

impl Dir {
    pub fn new(name: &str) -> Dir {
        let d = std::env::temp_dir().join(format!("specular-ai-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Dir(d)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Deterministic bytes.
pub fn bytes(n: usize, seed: u8) -> Vec<u8> {
    (0..n)
        .map(|i| {
            (i as u32)
                .wrapping_mul(2_654_435_761)
                .rotate_left(seed as u32) as u8
                ^ seed
        })
        .collect()
}

pub fn sha_hex(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

/// A manifest entry (a placeholder: file:// URLs) for a file written to `dir`.
pub fn entry(dir: &Path, feature: &str, id: &str, body: &[u8], urls: &[String]) -> String {
    let path = dir.join(format!("{id}.bin"));
    std::fs::write(&path, body).unwrap();
    let mut all: Vec<String> = urls.to_vec();
    all.push(format!("file://{}", path.display()));
    let urls = all
        .iter()
        .map(|u| format!("{u:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"
[[model]]
feature = "{feature}"
id = "{id}"
version = "1"
role = "test"
urls = [{urls}]
sha256 = "{sha}"
size = {size}
licence = "CC0-1.0"
licence_url = "https://models.invalid/cc0"
placeholder = true

[model.runtime]
format = "bin"
input = 128
dims = 512
"#,
        sha = sha_hex(body),
        size = body.len()
    )
}

/// A catalogue with `n` readable files (ids 1..=n); those in `thumbs` have a thumbnail
/// reference with content hash = id.
pub fn catalogue(path: &Path, n: i64, thumbs: impl Fn(i64) -> bool) {
    let conn = specular_library::ai::open(path).unwrap();
    conn.execute_batch(
        "INSERT INTO folders(id, path, added) VALUES (1, '/photos', 0);
         INSERT INTO dirs(id, folder_id, path) VALUES (1, 1, '/photos');",
    )
    .unwrap();
    for i in 1..=n {
        conn.execute(
            "INSERT INTO files(id, dir_id, name, ext, size, mtime_ns, state) VALUES (?1, 1, ?2, 'jpg', ?3, ?4, 1)",
            rusqlite::params![i, format!("{i}.jpg"), 1000 + i, 5000 + i],
        )
        .unwrap();
        if thumbs(i) {
            conn.execute(
                "INSERT INTO thumbs(file_id, content, classes) VALUES (?1, ?1, 1)",
                [i],
            )
            .unwrap();
        }
    }
}
