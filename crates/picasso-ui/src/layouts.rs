//! Saved layouts (Workspace › Layout › Save Layout and Load Layout): the sizes of the panels on
//! the pages under a name, in `layouts/<file>.txt` in the data folder. A layout file holds a
//! `name=...` line and then the same lines the preferences use for the sizes
//! (`crate::prefs::panels_text`), so it can be read and edited by hand.
//!
//! The browser has no data folder: there, no layouts are kept, as with the preferences.

use std::path::{Path, PathBuf};

use crate::panels::Panels;

/// The folder inside the data folder that the layouts are in.
const FOLDER: &str = "layouts";
/// A longer name is cut: no real layout has one.
const MAX_NAME: usize = 40;

/// The folder the layouts are in, made when it does not exist yet.
fn folder() -> Option<PathBuf> {
    let dir = crate::project_manager::data_dir()?.join(FOLDER);
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// The name as a file name: letters, digits, `-` and `_`; anything else becomes `-`. So a layout
/// called `../x` writes `x.txt` in the folder and not outside it.
fn slug(name: &str) -> String {
    let clean: String = name
        .chars()
        .take(MAX_NAME)
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let clean = clean.trim_matches('-').to_string();
    if clean.is_empty() {
        "layout".to_string()
    } else {
        clean
    }
}

/// The name a layout file gives itself, or its file name when the line is missing.
fn name_of(text: &str, file: &str) -> String {
    text.lines()
        .find_map(|l| l.trim_end_matches('\r').strip_prefix("name="))
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(file)
        .chars()
        .take(MAX_NAME)
        .collect()
}

/// The names of the layouts in `dir`, in the order they sort in.
fn names_in(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "txt"))
        .filter_map(|e| {
            let file = e.path().file_stem()?.to_string_lossy().into_owned();
            let text = std::fs::read_to_string(e.path()).unwrap_or_default();
            Some(name_of(&text, &file))
        })
        .take(64)
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

fn save_in(dir: &Path, name: &str, panels: &Panels) -> std::io::Result<()> {
    let text = format!("name={}\n{}", name.trim(), crate::prefs::panels_text(panels));
    std::fs::create_dir_all(dir).and_then(|()| std::fs::write(dir.join(slug(name) + ".txt"), text))
}

fn load_in(dir: &Path, name: &str) -> Option<Panels> {
    let text = std::fs::read_to_string(dir.join(slug(name) + ".txt")).ok()?;
    crate::prefs::panels_of(&text)
}

/// The names of the saved layouts, in the order they sort in.
pub(crate) fn names() -> Vec<String> {
    folder().map_or_else(Vec::new, |dir| names_in(&dir))
}

/// Saves `panels` as the layout `name`, replacing one of the same name. The error is the reason
/// it could not be written, to show as it is.
pub(crate) fn save(name: &str, panels: &Panels) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("a layout needs a name".into());
    }
    let dir = folder().ok_or("there is nowhere to save it on this computer")?;
    save_in(&dir, name, panels).map_err(|e| format!("the layout could not be saved: {e}"))
}

/// The panels of the saved layout `name`, if it is there.
pub(crate) fn load(name: &str) -> Option<Panels> {
    folder().and_then(|dir| load_in(&dir, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder of its own for one test, made fresh and left behind for the next run to replace.
    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("picasso_layouts_{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn layouts_read_back_as_they_were_saved() {
        let dir = scratch("round_trip");
        let mut panels = Panels::default();
        panels.set_edit_column(1, 600.0);
        panels.cut_upper = 400;
        panels.moved.insert("color gallery".into(), -35);
        save_in(&dir, "Editing", &panels).unwrap();
        assert_eq!(load_in(&dir, "Editing"), Some(panels.clone()));
        // The same name again replaces the first, and keeps its place in the list.
        let mut other = Panels::default();
        other.cut_pool = 800;
        save_in(&dir, "Editing", &other).unwrap();
        assert_eq!(load_in(&dir, "Editing"), Some(other.clone()));
        save_in(&dir, "Colour", &panels).unwrap();
        assert_eq!(names_in(&dir), ["Colour", "Editing"]);
        assert_eq!(load_in(&dir, "Colour"), Some(panels));
    }

    #[test]
    fn a_name_stays_inside_the_folder_and_a_broken_file_is_not_a_layout() {
        let dir = scratch("names");
        save_in(&dir, "../../outside", &Panels::default()).unwrap();
        assert_eq!(names_in(&dir), ["../../outside"]);
        // Nothing was written outside the folder.
        assert!(!dir.join("../outside.txt").exists());
        // A file without a readable `panels=` line is no layout.
        std::fs::write(dir.join("damaged.txt"), "name=Damaged\npanels=1,2,3\n").unwrap();
        assert_eq!(names_in(&dir), ["../../outside", "Damaged"]);
        assert_eq!(load_in(&dir, "Damaged"), None);
        assert_eq!(load_in(&dir, "Not here"), None);
        // A name that cannot be a file name at all still lands in the folder.
        assert_eq!(slug("///"), "layout");
        assert_eq!(slug("  a b  "), "a-b");
        assert_eq!(slug(""), "layout");
    }

    #[test]
    fn a_layout_carries_the_sizes_of_every_page() {
        let dir = scratch("every_page");
        let mut panels = Panels::default();
        for (i, id) in ["media files", "photo pool", "fx nodes", "deliver queue"]
            .into_iter()
            .enumerate()
        {
            panels.moved.insert(id.into(), i as i16 - 2);
        }
        save_in(&dir, "Mine", &panels).unwrap();
        let read = load_in(&dir, "Mine").unwrap();
        assert_eq!(read, panels, "a divider moved on any page comes back moved");
    }
}
