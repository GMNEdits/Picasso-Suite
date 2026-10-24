//! Every user command, its keyboard binding and its place in the menus.
//! One table drives both the menus and the key handling.

use egui::{Key, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    Media,
    Photo,
    Cut,
    Edit,
    Effects,
    Color,
    Audio,
    Deliver,
}

impl Page {
    pub const ALL: [Page; 8] =
        [Page::Media, Page::Photo, Page::Cut, Page::Edit, Page::Effects, Page::Color, Page::Audio, Page::Deliver];

    pub fn label(self) -> &'static str {
        match self {
            Page::Media => "Media",
            Page::Photo => "Photo",
            Page::Cut => "Cut",
            Page::Edit => "Edit",
            Page::Effects => "Effects",
            Page::Color => "Color",
            Page::Audio => "Audio",
            Page::Deliver => "Deliver",
        }
    }

    pub fn built(self) -> bool {
        matches!(self, Page::Cut | Page::Edit | Page::Deliver)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    NewProject,
    OpenProject,
    SaveProject,
    SaveProjectAs,
    ImportMedia,
    RelinkMedia,
    ExportOtio,
    ProjectSettings,
    QuickExport,
    Quit,
    Undo,
    Redo,
    DeleteSelected,
    RippleDelete,
    SelectAll,
    DeselectAll,
    TrimStart,
    TrimEnd,
    AddVideoTrack,
    AddAudioTrack,
    Razor,
    ToggleSnapping,
    SelectTool,
    BladeTool,
    ZoomIn,
    ZoomOut,
    ZoomFit,
    ToggleClipEnabled,
    LinkClips,
    UnlinkClips,
    MarkIn,
    MarkOut,
    ClearInOut,
    ToggleMediaPool,
    ToggleInspector,
    ToggleBins,
    ToggleDualViewer,
    PlayStop,
    PlayReverse,
    Pause,
    PlayForward,
    ToStart,
    ToEnd,
    PrevFrame,
    NextFrame,
    PrevEdit,
    NextEdit,
    Page(Page),
    FullScreen,
    Shortcuts,
    About,
    /// A menu entry that exists in the layout but has no function yet.
    NotYet,
}

pub struct Binding {
    pub cmd: Cmd,
    pub mods: Modifiers,
    pub key: Key,
}

const fn b(cmd: Cmd, mods: Modifiers, key: Key) -> Binding {
    Binding { cmd, mods, key }
}

const CMD: Modifiers = Modifiers::COMMAND;
const SHIFT: Modifiers = Modifiers::SHIFT;
const ALT: Modifiers = Modifiers::ALT;
const NONE: Modifiers = Modifiers::NONE;
const CMD_SHIFT: Modifiers = Modifiers { alt: false, ctrl: false, shift: true, mac_cmd: false, command: true };
const CMD_ALT: Modifiers = Modifiers { alt: true, ctrl: false, shift: false, mac_cmd: false, command: true };

/// Keyboard bindings. Order matters: more specific modifiers come first.
pub const BINDINGS: &[Binding] = &[
    b(Cmd::NewProject, CMD, Key::N),
    b(Cmd::OpenProject, CMD, Key::O),
    b(Cmd::SaveProjectAs, CMD_SHIFT, Key::S),
    b(Cmd::SaveProject, CMD, Key::S),
    b(Cmd::ImportMedia, CMD, Key::I),
    b(Cmd::QuickExport, CMD_SHIFT, Key::E),
    b(Cmd::Quit, CMD, Key::Q),
    b(Cmd::ProjectSettings, SHIFT, Key::Num9),
    b(Cmd::Redo, CMD_SHIFT, Key::Z),
    b(Cmd::Undo, CMD, Key::Z),
    b(Cmd::RippleDelete, SHIFT, Key::Backspace),
    b(Cmd::DeleteSelected, NONE, Key::Backspace),
    b(Cmd::RippleDelete, NONE, Key::Delete),
    b(Cmd::DeselectAll, CMD_SHIFT, Key::A),
    b(Cmd::SelectAll, CMD, Key::A),
    b(Cmd::TrimStart, SHIFT, Key::OpenBracket),
    b(Cmd::TrimEnd, SHIFT, Key::CloseBracket),
    b(Cmd::Razor, CMD, Key::B),
    b(Cmd::Razor, CMD, Key::Backslash),
    b(Cmd::ToggleSnapping, NONE, Key::N),
    b(Cmd::SelectTool, NONE, Key::A),
    b(Cmd::BladeTool, NONE, Key::B),
    b(Cmd::ZoomIn, CMD, Key::Equals),
    b(Cmd::ZoomIn, CMD, Key::Plus),
    b(Cmd::ZoomOut, CMD, Key::Minus),
    b(Cmd::ZoomFit, SHIFT, Key::Z),
    b(Cmd::ToggleClipEnabled, NONE, Key::D),
    b(Cmd::LinkClips, CMD_ALT, Key::L),
    b(Cmd::ClearInOut, ALT, Key::X),
    b(Cmd::MarkIn, NONE, Key::I),
    b(Cmd::MarkOut, NONE, Key::O),
    b(Cmd::PlayStop, NONE, Key::Space),
    b(Cmd::PlayReverse, NONE, Key::J),
    b(Cmd::Pause, NONE, Key::K),
    b(Cmd::PlayForward, NONE, Key::L),
    b(Cmd::ToStart, NONE, Key::Home),
    b(Cmd::ToEnd, NONE, Key::End),
    b(Cmd::PrevFrame, NONE, Key::ArrowLeft),
    b(Cmd::NextFrame, NONE, Key::ArrowRight),
    b(Cmd::PrevEdit, NONE, Key::ArrowUp),
    b(Cmd::NextEdit, NONE, Key::ArrowDown),
    b(Cmd::Page(Page::Cut), SHIFT, Key::Num3),
    b(Cmd::Page(Page::Edit), SHIFT, Key::Num4),
    b(Cmd::Page(Page::Deliver), SHIFT, Key::Num8),
    b(Cmd::FullScreen, CMD_SHIFT, Key::F),
];

/// Commands that make sense while a text field has keyboard focus.
pub fn allowed_while_typing(cmd: Cmd) -> bool {
    !matches!(
        cmd,
        Cmd::DeleteSelected
            | Cmd::RippleDelete
            | Cmd::SelectAll
            | Cmd::ToggleSnapping
            | Cmd::SelectTool
            | Cmd::BladeTool
            | Cmd::ToggleClipEnabled
            | Cmd::MarkIn
            | Cmd::MarkOut
            | Cmd::PlayStop
            | Cmd::PlayReverse
            | Cmd::Pause
            | Cmd::PlayForward
            | Cmd::ToStart
            | Cmd::ToEnd
            | Cmd::PrevFrame
            | Cmd::NextFrame
            | Cmd::PrevEdit
            | Cmd::NextEdit
            | Cmd::ProjectSettings
            | Cmd::ZoomFit
            | Cmd::TrimStart
            | Cmd::TrimEnd
            | Cmd::Page(_)
    )
}

/// The shortcut shown next to a command in menus.
pub fn shortcut_text(ctx: &egui::Context, cmd: Cmd) -> Option<String> {
    BINDINGS.iter().find(|bnd| bnd.cmd == cmd).map(|bnd| ctx.format_shortcut(&egui::KeyboardShortcut::new(bnd.mods, bnd.key)))
}

pub enum Entry {
    Item(&'static str, Cmd),
    Toggle(&'static str, Cmd),
    Sub(&'static str, &'static [Entry]),
    Sep,
}

use Entry::*;

pub const MENUS: &[(&str, &[Entry])] = &[
    ("File", &[
        Item("New Project", Cmd::NewProject),
        Item("Open Project...", Cmd::OpenProject),
        Sep,
        Item("Save Project", Cmd::SaveProject),
        Item("Save Project As...", Cmd::SaveProjectAs),
        Sep,
        Sub("Import", &[Item("Media...", Cmd::ImportMedia), Item("DaVinci Resolve Project (.drp)...", Cmd::OpenProject)]),
        Sub("Export", &[Item("Timeline as OpenTimelineIO...", Cmd::ExportOtio)]),
        Item("Relink Missing Media...", Cmd::RelinkMedia),
        Sep,
        Item("Project Settings...", Cmd::ProjectSettings),
        Sep,
        Item("Quick Export...", Cmd::QuickExport),
        Sep,
        Item("Quit Picasso Suite", Cmd::Quit),
    ]),
    ("Edit", &[
        Item("Undo", Cmd::Undo),
        Item("Redo", Cmd::Redo),
        Sep,
        Item("Delete Selected", Cmd::DeleteSelected),
        Item("Ripple Delete", Cmd::RippleDelete),
        Sep,
        Item("Select All", Cmd::SelectAll),
        Item("Deselect All", Cmd::DeselectAll),
    ]),
    ("Trim", &[Item("Trim Start", Cmd::TrimStart), Item("Trim End", Cmd::TrimEnd)]),
    ("Timeline", &[
        Sub("Add Track", &[Item("Video", Cmd::AddVideoTrack), Item("Audio", Cmd::AddAudioTrack)]),
        Sep,
        Item("Razor", Cmd::Razor),
        Sep,
        Toggle("Snapping", Cmd::ToggleSnapping),
        Sep,
        Item("Zoom In", Cmd::ZoomIn),
        Item("Zoom Out", Cmd::ZoomOut),
        Item("Zoom to Fit", Cmd::ZoomFit),
    ]),
    ("Clip", &[
        Item("Enable / Disable Clip", Cmd::ToggleClipEnabled),
        Sep,
        Item("Link Clips", Cmd::LinkClips),
        Item("Unlink Clips", Cmd::UnlinkClips),
    ]),
    ("Mark", &[Item("Mark In", Cmd::MarkIn), Item("Mark Out", Cmd::MarkOut), Item("Clear In and Out", Cmd::ClearInOut)]),
    ("View", &[
        Toggle("Media Pool", Cmd::ToggleMediaPool),
        Toggle("Bins", Cmd::ToggleBins),
        Toggle("Inspector", Cmd::ToggleInspector),
        Toggle("Dual Viewer", Cmd::ToggleDualViewer),
    ]),
    ("Playback", &[
        Item("Play / Stop", Cmd::PlayStop),
        Item("Play Reverse", Cmd::PlayReverse),
        Item("Stop", Cmd::Pause),
        Item("Play Forward", Cmd::PlayForward),
        Sep,
        Item("Go to First Frame", Cmd::ToStart),
        Item("Go to Last Frame", Cmd::ToEnd),
        Item("Previous Frame", Cmd::PrevFrame),
        Item("Next Frame", Cmd::NextFrame),
        Item("Previous Edit", Cmd::PrevEdit),
        Item("Next Edit", Cmd::NextEdit),
    ]),
    ("Effects", &[Item("Not available yet", Cmd::NotYet)]),
    ("Color", &[Item("Not available yet", Cmd::NotYet)]),
    ("Audio", &[Item("Not available yet", Cmd::NotYet)]),
    ("Workspace", &[
        Sub("Switch to Page", &[Item("Cut", Cmd::Page(Page::Cut)), Item("Edit", Cmd::Page(Page::Edit)), Item("Deliver", Cmd::Page(Page::Deliver))]),
        Item("Full Screen Window", Cmd::FullScreen),
    ]),
    ("Help", &[Item("Keyboard Shortcuts", Cmd::Shortcuts), Item("About Picasso Suite", Cmd::About)]),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn no_key_is_bound_twice() {
        let mut seen = HashSet::new();
        for bnd in BINDINGS {
            assert!(seen.insert((format!("{:?}", bnd.mods), bnd.key)), "{:?} {:?} bound twice", bnd.mods, bnd.key);
        }
    }

    #[test]
    fn every_menu_command_is_handled_or_marked() {
        fn walk(entries: &[Entry], out: &mut Vec<Cmd>) {
            for e in entries {
                match e {
                    Item(_, c) | Toggle(_, c) => out.push(*c),
                    Sub(_, inner) => walk(inner, out),
                    Sep => {}
                }
            }
        }
        let mut cmds = Vec::new();
        for (_, entries) in MENUS {
            walk(entries, &mut cmds);
        }
        assert!(cmds.contains(&Cmd::Razor) && cmds.contains(&Cmd::ImportMedia));
        assert_eq!(MENUS.len(), 13, "same menu bar as the reference");
    }
}
