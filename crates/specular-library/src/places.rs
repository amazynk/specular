//! Place names for search (DESIGN §6.7 "Places from GPS").
//!
//! The facts pass files every photo with a position under the nearest populated place
//! (`places`, one row per town: [`crate::facts::place_of`]). A search reads that small table
//! once, names each place in the viewer's language (the country through CLDR, so "Portugal",
//! "Portogallo" and "PT" all find Lisbon), and matches words against the names here rather
//! than in the full-text index: the files themselves are then selected by `place_id`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use specular_metadata::{LanguageIdentifier, Localizer};
use specular_query::Fold;

/// The places of the catalogue, each as the lines a `place:` term and plain words search:
/// town, region, district, the country's name in English and in the viewer's language, and
/// its ISO code.
#[derive(Debug, Default)]
pub struct Places {
    text: HashMap<i64, String>,
}

impl Places {
    /// Reads every place, naming countries in `lang` as well as in English.
    pub fn load(c: &Connection, lang: Option<&LanguageIdentifier>) -> rusqlite::Result<Places> {
        let local = lang.map(localizer);
        let english = Localizer::english_shared();
        let mut st = c.prepare_cached("SELECT id, city, admin1, admin2, cc FROM places")?;
        let mut q = st.query([])?;
        let mut text = HashMap::new();
        while let Some(r) = q.next()? {
            let parts: [String; 4] = [r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?];
            let [city, admin1, admin2, cc] = parts;
            let mut lines: Vec<String> = vec![city, admin1, admin2];
            if !cc.is_empty() {
                lines.push(english.country_name(&cc));
                if let Some(l) = &local {
                    lines.push(l.country_name(&cc));
                }
                lines.push(cc);
            }
            let mut t = String::new();
            for l in lines {
                let l = l.trim();
                // A line once (the region often is the town; English is often the local name).
                if l.is_empty() || t.lines().any(|x| x == l) {
                    continue;
                }
                if !t.is_empty() {
                    t.push('\n');
                }
                t.push_str(l);
            }
            text.insert(r.get(0)?, t);
        }
        Ok(Places { text })
    }

    /// The text of a place (None: no such place).
    pub fn text(&self, id: i64) -> Option<&str> {
        self.text.get(&id).map(String::as_str)
    }

    /// The places whose text contains one of the folded `needles`, ascending.
    pub fn matching(&self, needles: &[String], fold: Fold) -> Vec<i64> {
        let mut v: Vec<i64> = self
            .text
            .iter()
            .filter(|(_, t)| {
                let t = fold(t);
                needles.iter().any(|n| t.contains(n.as_str()))
            })
            .map(|(id, _)| *id)
            .collect();
        v.sort_unstable();
        v
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// The localizer of `lang`, kept between searches (making one loads its CLDR data).
fn localizer(lang: &LanguageIdentifier) -> Arc<Localizer> {
    static LAST: Mutex<Option<Arc<Localizer>>> = Mutex::new(None);
    let mut last = LAST
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(l) = last.as_ref().filter(|l| l.langid() == lang) {
        return l.clone();
    }
    let l = Arc::new(Localizer::new(lang.clone()));
    *last = Some(l.clone());
    l
}
