//! Cross-platform media path resolution.
//!
//! Projects move between machines: a `.drp` made on macOS points at
//! `/Volumes/Media/...`, one from Windows at `D:\Footage\...`, one from Linux at
//! `/mnt/...`. Paths are parsed independently of the host OS and resolved by,
//! in order: the original path, user path mappings, then a filename search
//! below a set of root folders (preferring the most matching parent folders).

use picasso_core::Project;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Splits a path from any OS into components. `C:\a\b` → ["C:", "a", "b"],
/// `/Volumes/x/y` → ["Volumes", "x", "y"], `\\server\share\f` → ["server", "share", "f"].
pub fn components(path: &str) -> Vec<String> {
    path.split(['/', '\\']).filter(|c| !c.is_empty() && *c != ".").map(str::to_owned).collect()
}

pub fn file_name(path: &str) -> Option<String> {
    components(path).pop()
}

/// A prefix rewrite such as `D:\Footage` → `/mnt/footage`.
#[derive(Clone, Debug, PartialEq)]
pub struct PathMapping {
    pub from: String,
    pub to: PathBuf,
}

impl PathMapping {
    /// Case-insensitive component-prefix match.
    pub fn apply(&self, original: &str) -> Option<PathBuf> {
        let from = components(&self.from);
        let orig = components(original);
        if from.is_empty() || orig.len() < from.len() {
            return None;
        }
        if !from.iter().zip(&orig).all(|(a, b)| a.eq_ignore_ascii_case(b)) {
            return None;
        }
        let mut out = self.to.clone();
        for c in &orig[from.len()..] {
            out.push(c);
        }
        Some(out)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Relinker {
    pub mappings: Vec<PathMapping>,
    pub roots: Vec<PathBuf>,
    pub max_depth: usize,
}

impl Relinker {
    pub fn with_roots(roots: Vec<PathBuf>) -> Self {
        Self { mappings: Vec::new(), roots, max_depth: 6 }
    }

    /// Searches next to the project file (and its parent), which covers
    /// projects shipped together with their media folder.
    pub fn for_project_file(project_file: &Path) -> Self {
        let mut roots = Vec::new();
        if let Some(dir) = project_file.parent().filter(|d| !d.as_os_str().is_empty()) {
            roots.push(dir.to_path_buf());
            if let Some(up) = dir.parent().filter(|d| !d.as_os_str().is_empty()) {
                roots.push(up.to_path_buf());
            }
        }
        let mut r = Self::with_roots(roots);
        r.mappings = mappings_from_env();
        r
    }

    /// Resolves an original path written on any OS to an existing local file.
    pub fn resolve(&self, original: &str) -> Option<PathBuf> {
        let direct = PathBuf::from(original);
        if direct.is_file() {
            return Some(direct);
        }
        // Same path with this OS's separators (e.g. relative paths).
        let native: PathBuf = components(original).iter().collect();
        if !original.contains(':') && original.starts_with(['/', '\\']) {
            let rooted = Path::new("/").join(&native);
            if rooted.is_file() {
                return Some(rooted);
            }
        }
        if let Some(p) = self.mappings.iter().filter_map(|m| m.apply(original)).find(|p| p.is_file()) {
            return Some(p);
        }
        self.search(original)
    }

    fn search(&self, original: &str) -> Option<PathBuf> {
        let wanted = components(original);
        let name = wanted.last()?.to_lowercase();
        let mut best: Option<(usize, PathBuf)> = None;
        for root in &self.roots {
            let mut stack = vec![(root.clone(), 0usize)];
            while let Some((dir, depth)) = stack.pop() {
                let Ok(entries) = std::fs::read_dir(&dir) else { continue };
                for e in entries.flatten() {
                    let p = e.path();
                    let Ok(ft) = e.file_type() else { continue };
                    if ft.is_dir() {
                        if depth < self.max_depth {
                            stack.push((p, depth + 1));
                        }
                    } else if e.file_name().to_string_lossy().to_lowercase() == name {
                        let score = trailing_match(&wanted, &p);
                        if best.as_ref().is_none_or(|(s, _)| score > *s) {
                            best = Some((score, p));
                        }
                    }
                }
            }
        }
        best.map(|(_, p)| p)
    }
}

/// How many trailing components (file name + parent folders) match.
fn trailing_match(wanted: &[String], candidate: &Path) -> usize {
    let cand = components(&candidate.to_string_lossy());
    wanted.iter().rev().zip(cand.iter().rev()).take_while(|(a, b)| a.eq_ignore_ascii_case(b)).count()
}

/// `PICASSO_PATH_MAP="D:\Footage=/mnt/footage;/Volumes/Media=/media/me/Media"`.
pub fn mappings_from_env() -> Vec<PathMapping> {
    std::env::var("PICASSO_PATH_MAP")
        .ok()
        .map(|v| {
            v.split(';')
                .filter_map(|pair| pair.split_once('='))
                .map(|(from, to)| PathMapping { from: from.trim().into(), to: PathBuf::from(to.trim()) })
                .collect()
        })
        .unwrap_or_default()
}

/// Relinks media whose files are missing. Returns how many were fixed.
pub fn relink_missing(project: &mut Project, relinker: &Relinker) -> usize {
    let mut fixed = 0;
    let mut cache: HashMap<String, Option<PathBuf>> = HashMap::new();
    for m in project.media.iter_mut().filter(|m| m.sequence.is_none() && !m.path.is_file()) {
        let original = m.path.to_string_lossy().into_owned();
        let found = cache.entry(original.clone()).or_insert_with(|| relinker.resolve(&original)).clone();
        if let Some(p) = found {
            if let Ok(probed) = picasso_media::probe(&p, project.settings.fps) {
                m.info = probed.info;
            }
            m.path = p;
            fixed += 1;
        }
    }
    fixed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn components_of_any_os() {
        assert_eq!(components(r"C:\Users\me\clip.mov"), ["C:", "Users", "me", "clip.mov"]);
        assert_eq!(components("/Volumes/Media/A001/clip.mov"), ["Volumes", "Media", "A001", "clip.mov"]);
        assert_eq!(components(r"\\nas\share\x.mxf"), ["nas", "share", "x.mxf"]);
    }

    #[test]
    fn mapping_rewrites_prefix_case_insensitively() {
        let m = PathMapping { from: r"d:\footage".into(), to: PathBuf::from("/mnt/footage") };
        assert_eq!(m.apply(r"D:\Footage\Day1\A.mov"), Some(PathBuf::from("/mnt/footage").join("Day1").join("A.mov")));
        assert_eq!(m.apply(r"E:\Footage\A.mov"), None);
    }

    #[test]
    fn search_prefers_matching_parent_folders() {
        let dir = tempfile::tempdir().unwrap();
        for sub in ["Day1", "Day2"] {
            std::fs::create_dir_all(dir.path().join("Footage").join(sub)).unwrap();
            std::fs::write(dir.path().join("Footage").join(sub).join("A001.mov"), b"x").unwrap();
        }
        let r = Relinker::with_roots(vec![dir.path().to_path_buf()]);
        let found = r.resolve("/Volumes/Shoot/Footage/Day2/A001.mov").unwrap();
        assert!(found.ends_with(Path::new("Footage").join("Day2").join("A001.mov")));
        assert_eq!(r.resolve(r"C:\nothing\here.mov"), None);
    }
}
