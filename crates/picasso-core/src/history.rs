//! Snapshot-based undo/redo. Projects are small (media metadata plus clip
//! lists), so whole-project snapshots are simple and robust.

use crate::model::Project;

const MAX_STEPS: usize = 200;

#[derive(Default)]
pub struct History {
    undo: Vec<(String, Project)>,
    redo: Vec<(String, Project)>,
}

impl History {
    /// Call before mutating `project`; `label` names the action for the UI.
    pub fn checkpoint(&mut self, label: &str, project: &Project) {
        self.undo.push((label.to_owned(), project.clone()));
        if self.undo.len() > MAX_STEPS {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Drops the most recent checkpoint, for when the edit turned out to be a no-op.
    pub fn discard_last(&mut self) {
        self.undo.pop();
    }

    pub fn undo(&mut self, project: &mut Project) -> Option<String> {
        let (label, prev) = self.undo.pop()?;
        self.redo.push((label.clone(), std::mem::replace(project, prev)));
        Some(label)
    }

    pub fn redo(&mut self, project: &mut Project) -> Option<String> {
        let (label, next) = self.redo.pop()?;
        self.undo.push((label.clone(), std::mem::replace(project, next)));
        Some(label)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_roundtrip() {
        let mut p = Project::default();
        let mut h = History::default();
        h.checkpoint("rename", &p);
        p.name = "B".into();
        assert_eq!(h.undo(&mut p).as_deref(), Some("rename"));
        assert_eq!(p.name, "Untitled");
        h.redo(&mut p);
        assert_eq!(p.name, "B");
        assert!(!h.can_redo());
    }
}
