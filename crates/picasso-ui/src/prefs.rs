//! The settings of this computer rather than of a project (System Preferences and Keyboard
//! Customization), in `preferences.txt` in the data folder: `hardware_decoding=on` and
//! `audio_scrubbing=on` lines, a `key<TAB>Undo<TAB>Ctrl+U` line for each command whose keys were
//! changed, with the keys it has instead of the menus' (none: no key), and a
//! `panels=147,457,1186,474,960,519` line with the sizes of the panels whose dividers were
//! dragged (`crate::panels`). Lines that cannot be read are skipped, so a file from another
//! version still opens.

use std::collections::HashMap;

use crate::menu::{self, Act};
use crate::panels::Panels;

#[cfg_attr(test, allow(dead_code))] // the tests never touch the real file
const FILE: &str = "preferences.txt";
/// More lines than any real file has: the rest of a damaged or hostile file is not read.
const MAX_LINES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Preferences {
    /// Decode H.264, H.265, AV1, VP9, MPEG-2 and VC-1 on the graphics card where it can.
    pub hardware_decoding: bool,
    /// Play short pieces of the sound while the playhead is dragged (the Edit menu's
    /// "Enable / Disable Audio Scrubbing").
    pub audio_scrubbing: bool,
    /// The commands whose keys were changed, and their keys.
    pub keys: HashMap<Act, Vec<String>>,
    /// The sizes of the panels on the pages.
    pub panels: Panels,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            hardware_decoding: true,
            audio_scrubbing: true,
            keys: HashMap::new(),
            panels: Panels::default(),
        }
    }
}

impl Preferences {
    pub(crate) fn parse(text: &str) -> Self {
        let mut prefs = Self::default();
        for line in text.lines().take(MAX_LINES) {
            let line = line.trim_end_matches('\r');
            if let Some(rest) = line.strip_prefix("key\t") {
                let mut parts = rest.split('\t');
                let Some(act) = parts.next().and_then(menu::act_named) else {
                    continue;
                };
                let keys = parts
                    .filter(|k| menu::parse_keys(k).is_some())
                    .take(8)
                    .map(str::to_string)
                    .collect();
                prefs.keys.insert(act, keys);
            } else if let Some(rest) = line.strip_prefix("panel\t") {
                let Some((name, by)) = rest.split_once('\t') else {
                    continue;
                };
                let moved = &mut prefs.panels.moved;
                if let Ok(by) = by.trim().parse::<i16>() {
                    if name.len() <= 40 && moved.len() < 64 {
                        moved.insert(name.to_string(), by);
                    }
                }
            } else if let Some((name, value)) = line.split_once('=') {
                match name.trim() {
                    "hardware_decoding" => prefs.hardware_decoding = value.trim() != "off",
                    "audio_scrubbing" => prefs.audio_scrubbing = value.trim() != "off",
                    "panels" => {
                        if let Some(panels) = parse_panels(value) {
                            let moved = std::mem::take(&mut prefs.panels.moved);
                            prefs.panels = Panels { moved, ..panels };
                        }
                    }
                    _ => {}
                }
            }
        }
        prefs
    }

    pub(crate) fn text(&self) -> String {
        let mut out = format!(
            "hardware_decoding={}\naudio_scrubbing={}\n",
            if self.hardware_decoding { "on" } else { "off" },
            if self.audio_scrubbing { "on" } else { "off" }
        );
        out.push_str(&panels_text(&self.panels));
        let mut keys: Vec<(String, &Vec<String>)> = self
            .keys
            .iter()
            .map(|(act, keys)| (menu::act_name(*act), keys))
            .collect();
        keys.sort();
        for (name, keys) in keys {
            out.push_str("key\t");
            out.push_str(&name);
            for k in keys {
                out.push('\t');
                out.push_str(&k.replace(['\t', '\n'], ""));
            }
            out.push('\n');
        }
        out
    }

    /// Puts the preferences to work: the keys, and hardware decoding for the files opened next.
    pub(crate) fn apply(&self) {
        menu::set_custom_keys(self.keys.clone());
        #[cfg(not(target_arch = "wasm32"))]
        picasso_decode::hw::set_hardware_decoding(self.hardware_decoding);
    }

    /// Reads the file in the data folder; the defaults when there is none.
    #[cfg_attr(test, allow(dead_code))]
    pub(crate) fn load() -> Self {
        crate::project_manager::data_dir()
            .and_then(|dir| std::fs::read_to_string(dir.join(FILE)).ok())
            .map_or_else(Self::default, |text| Self::parse(&text))
    }

    #[cfg_attr(test, allow(dead_code))]
    pub(crate) fn save(&self) {
        let Some(dir) = crate::project_manager::data_dir() else {
            return;
        };
        if let Err(e) =
            std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(dir.join(FILE), self.text()))
        {
            tracing::warn!("could not save the preferences: {e}");
        }
    }
}

/// The six numbers of a `panels=` line; nothing unless all six read.
fn parse_panels(value: &str) -> Option<Panels> {
    let n: Vec<u16> = value
        .split(',')
        .map(|v| v.trim().parse().ok())
        .collect::<Option<_>>()?;
    let [a, b, c, viewers, pool, upper] = n[..] else {
        return None;
    };
    Some(
        Panels {
            edit_columns: [a, b, c],
            edit_viewers: viewers,
            cut_pool: pool,
            cut_upper: upper,
            moved: Default::default(),
        }
        .checked(),
    )
}

/// The lines that hold the panels' sizes: a `panels=` line, then a `panel` line for each divider
/// that was moved. A saved layout (`crate::layouts`) is these lines and nothing else.
pub(crate) fn panels_text(p: &Panels) -> String {
    let [a, b, c] = p.edit_columns;
    let mut out = format!(
        "panels={a},{b},{c},{},{},{}\n",
        p.edit_viewers, p.cut_pool, p.cut_upper
    );
    for (name, by) in &p.moved {
        out.push_str(&format!("panel\t{}\t{by}\n", name.replace(['\t', '\n'], "")));
    }
    out
}

/// The panels' sizes in lines that `panels_text` wrote; `None` when the `panels=` line is missing
/// or cannot be read (a damaged or hand-edited file), so a broken layout is not taken for the
/// default sizes.
pub(crate) fn panels_of(text: &str) -> Option<Panels> {
    let value = text
        .lines()
        .take(MAX_LINES)
        .find_map(|l| l.trim_end_matches('\r').strip_prefix("panels="))?;
    parse_panels(value)?;
    Some(Preferences::parse(text).panels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_read_back_as_they_are_written() {
        let mut prefs = Preferences {
            hardware_decoding: false,
            audio_scrubbing: false,
            ..Preferences::default()
        };
        prefs.keys.insert(Act::Undo, vec!["Ctrl+U".into()]);
        prefs.panels.set_edit_column(1, 600.0);
        prefs.panels.cut_upper = 400;
        prefs.panels.moved.insert("color gallery".into(), -35);
        prefs.keys.insert(Act::Redo, Vec::new());
        prefs
            .keys
            .insert(Act::TogglePlay, vec!["K".into(), "Ctrl+[".into()]);
        let text = prefs.text();
        assert!(
            text.starts_with("hardware_decoding=off\naudio_scrubbing=off\n"),
            "{text}"
        );
        assert_eq!(Preferences::parse(&text), prefs);
        // Windows line ends, too.
        assert_eq!(Preferences::parse(&text.replace('\n', "\r\n")), prefs);
    }

    #[test]
    fn what_cannot_be_read_is_left_out() {
        let prefs = Preferences::parse(
            "hardware_decoding=on\nkey\tNoSuchCommand\tCtrl+Q\nkey\tUndo\tCtrl+Nonsense\tCtrl+U\nrubbish\n=\n",
        );
        assert!(prefs.hardware_decoding);
        assert_eq!(prefs.keys.len(), 1);
        assert_eq!(
            prefs.keys.get(&Act::Undo),
            Some(&vec!["Ctrl+U".to_string()])
        );
        assert_eq!(Preferences::parse(""), Preferences::default());
        // Panel sizes: all six numbers or none, and in range.
        assert_eq!(
            Preferences::parse("panels=1,2,3").panels,
            Preferences::default().panels
        );
        assert_eq!(
            Preferences::parse("panels=147,457,1186,9999,960,519")
                .panels
                .edit_viewers,
            750
        );
    }
}
