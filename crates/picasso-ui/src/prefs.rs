//! The settings of this computer rather than of a project (System Preferences and Keyboard
//! Customization), in `preferences.txt` in the data folder: a `hardware_decoding=on` line, and a
//! `key<TAB>Undo<TAB>Ctrl+U` line for each command whose keys were changed, with the keys it has
//! instead of the menus' (none: no key). Lines
//! that cannot be read are skipped, so a file from another version still opens.

use std::collections::HashMap;

use crate::menu::{self, Act};

#[cfg_attr(test, allow(dead_code))] // the tests never touch the real file
const FILE: &str = "preferences.txt";
/// More lines than any real file has: the rest of a damaged or hostile file is not read.
const MAX_LINES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Preferences {
    /// Decode H.264, H.265, AV1, VP9, MPEG-2 and VC-1 on the graphics card where it can.
    pub hardware_decoding: bool,
    /// The commands whose keys were changed, and their keys.
    pub keys: HashMap<Act, Vec<String>>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            hardware_decoding: true,
            keys: HashMap::new(),
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
            } else if let Some((name, value)) = line.split_once('=') {
                if name.trim() == "hardware_decoding" {
                    prefs.hardware_decoding = value.trim() != "off";
                }
            }
        }
        prefs
    }

    pub(crate) fn text(&self) -> String {
        let mut out = format!(
            "hardware_decoding={}\n",
            if self.hardware_decoding { "on" } else { "off" }
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_read_back_as_they_are_written() {
        let mut prefs = Preferences {
            hardware_decoding: false,
            ..Preferences::default()
        };
        prefs.keys.insert(Act::Undo, vec!["Ctrl+U".into()]);
        prefs.keys.insert(Act::Redo, Vec::new());
        prefs
            .keys
            .insert(Act::TogglePlay, vec!["K".into(), "Ctrl+[".into()]);
        let text = prefs.text();
        assert!(text.starts_with("hardware_decoding=off\n"), "{text}");
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
    }
}
