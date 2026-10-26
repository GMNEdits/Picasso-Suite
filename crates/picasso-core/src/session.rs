//! An open project with its undo history.

use crate::command::{Command, Outcome};
use crate::error::EditError;
use crate::project::Project;

/// How many steps can be undone.
const MAX_UNDO: usize = 200;

/// A project and the steps that led to it.
///
/// Every change goes through [`Session::dispatch`]. A drag of a slider is one step, not one per
/// frame: [`Session::dispatch_gesture`] keeps a gesture open until [`Session::end_gesture`].
#[derive(Debug)]
pub struct Session {
    project: Project,
    undo: Vec<(&'static str, Project)>,
    redo: Vec<(&'static str, Project)>,
    gesture_open: bool,
    revision: u64,
    /// Whether the project differs from what was last saved or opened.
    dirty: bool,
}

impl Session {
    pub fn new(project: Project) -> Self {
        Self {
            project,
            undo: Vec::new(),
            redo: Vec::new(),
            gesture_open: false,
            revision: 0,
            dirty: false,
        }
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    /// Changes whenever the project does; a cache of renders is valid while it stays the same.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    /// Runs a command as one undo step.
    pub fn dispatch(&mut self, command: Command) -> Result<Outcome, EditError> {
        self.gesture_open = false;
        self.run(command, true)
    }

    /// Runs a command as part of a gesture: the first one of a gesture is an undo step, the
    /// following ones until [`Session::end_gesture`] change the project without adding steps.
    pub fn dispatch_gesture(&mut self, command: Command) -> Result<Outcome, EditError> {
        let new_step = !self.gesture_open;
        let outcome = self.run(command, new_step)?;
        self.gesture_open = true;
        Ok(outcome)
    }

    /// Runs several commands as one undo step. If one is refused, none of them is applied and
    /// the project is exactly as it was.
    pub fn dispatch_all(&mut self, commands: Vec<Command>) -> Result<Vec<Outcome>, EditError> {
        self.dispatch_all_then(commands, |_| Vec::new())
    }

    /// Like [`Session::dispatch_all`], then runs the commands `then` makes from what the first
    /// ones made (linking pasted clips, say), all in the same undo step.
    pub fn dispatch_all_then(
        &mut self,
        commands: Vec<Command>,
        then: impl FnOnce(&[Outcome]) -> Vec<Command>,
    ) -> Result<Vec<Outcome>, EditError> {
        self.gesture_open = false;
        let Some(label) = commands.first().map(Command::label) else {
            return Ok(Vec::new());
        };
        let before = self.project.clone();
        let mut outcomes = Vec::with_capacity(commands.len());
        for command in commands {
            match command.apply(&mut self.project) {
                Ok(outcome) => outcomes.push(outcome),
                Err(e) => {
                    self.project = before;
                    return Err(e);
                }
            }
        }
        for command in then(&outcomes) {
            if let Err(e) = command.apply(&mut self.project) {
                self.project = before;
                return Err(e);
            }
        }
        self.undo.push((label, before));
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.revision += 1;
        self.dirty = true;
        Ok(outcomes)
    }

    pub fn end_gesture(&mut self) {
        self.gesture_open = false;
    }

    fn run(&mut self, command: Command, new_step: bool) -> Result<Outcome, EditError> {
        let label = command.label();
        let before = new_step.then(|| self.project.clone());
        // Commands validate before they change anything, so an `Err` leaves `project` alone.
        let outcome = command.apply(&mut self.project)?;
        if let Some(before) = before {
            self.undo.push((label, before));
            if self.undo.len() > MAX_UNDO {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
        self.revision += 1;
        self.dirty = true;
        Ok(outcome)
    }

    /// The label of the step [`Session::undo`] would take back.
    pub fn undo_label(&self) -> Option<&'static str> {
        self.undo.last().map(|(l, _)| *l)
    }

    pub fn redo_label(&self) -> Option<&'static str> {
        self.redo.last().map(|(l, _)| *l)
    }

    pub fn undo(&mut self) -> bool {
        self.gesture_open = false;
        let Some((label, previous)) = self.undo.pop() else {
            return false;
        };
        self.redo
            .push((label, std::mem::replace(&mut self.project, previous)));
        self.revision += 1;
        self.dirty = true;
        true
    }

    pub fn redo(&mut self) -> bool {
        self.gesture_open = false;
        let Some((label, next)) = self.redo.pop() else {
            return false;
        };
        self.undo
            .push((label, std::mem::replace(&mut self.project, next)));
        self.revision += 1;
        self.dirty = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::ClipId;
    use picasso_color::ColorGrade;

    fn session() -> (Session, ClipId) {
        let p = Project::demo();
        let id = p.tracks[0].clips[0].id;
        (Session::new(p), id)
    }

    #[test]
    fn undo_and_redo_restore_the_project() {
        let (mut s, a) = session();
        let original = s.project().clone();
        s.dispatch(Command::Delete { clip: a }).unwrap();
        assert_ne!(*s.project(), original);
        assert_eq!(s.undo_label(), Some("Delete clip"));
        assert!(s.undo());
        assert_eq!(*s.project(), original);
        assert_eq!(s.redo_label(), Some("Delete clip"));
        assert!(s.redo());
        assert!(s.project().clip(a).is_none());
        assert!(!s.redo());
    }

    #[test]
    fn a_refused_command_adds_no_step_and_keeps_the_revision() {
        let (mut s, _) = session();
        let revision = s.revision();
        assert!(s.dispatch(Command::Delete { clip: ClipId(404) }).is_err());
        assert_eq!(s.revision(), revision);
        assert!(!s.undo());
        assert!(!s.is_dirty());
    }

    #[test]
    fn a_new_command_clears_redo() {
        let (mut s, a) = session();
        s.dispatch(Command::SetOpacity {
            clip: a,
            opacity: 0.5,
        })
        .unwrap();
        s.undo();
        s.dispatch(Command::SetOpacity {
            clip: a,
            opacity: 0.2,
        })
        .unwrap();
        assert!(!s.redo());
    }

    #[test]
    fn a_gesture_is_one_undo_step() {
        let (mut s, a) = session();
        for v in [0.9, 0.8, 0.7, 0.6] {
            s.dispatch_gesture(Command::SetOpacity {
                clip: a,
                opacity: v,
            })
            .unwrap();
        }
        s.end_gesture();
        s.dispatch_gesture(Command::SetOpacity {
            clip: a,
            opacity: 0.3,
        })
        .unwrap();
        s.end_gesture();
        assert_eq!(s.project().clip(a).unwrap().1.opacity, 0.3);
        s.undo();
        assert_eq!(s.project().clip(a).unwrap().1.opacity, 0.6);
        s.undo();
        assert_eq!(s.project().clip(a).unwrap().1.opacity, 1.0);
        assert!(!s.undo());
    }

    #[test]
    fn a_normal_command_ends_a_gesture() {
        let (mut s, a) = session();
        s.dispatch_gesture(Command::SetOpacity {
            clip: a,
            opacity: 0.5,
        })
        .unwrap();
        s.dispatch(Command::SetGrade {
            clip: a,
            grade: ColorGrade {
                saturation: 0.0,
                ..Default::default()
            },
            node: 0,
        })
        .unwrap();
        s.dispatch_gesture(Command::SetOpacity {
            clip: a,
            opacity: 0.4,
        })
        .unwrap();
        s.undo();
        assert_eq!(s.project().clip(a).unwrap().1.opacity, 0.5);
        s.undo();
        assert_eq!(s.project().clip(a).unwrap().1.grade.saturation, 1.0);
    }

    #[test]
    fn history_is_bounded_and_dirty_tracks_saves() {
        let (mut s, a) = session();
        for i in 0..(MAX_UNDO + 20) {
            s.dispatch(Command::SetOpacity {
                clip: a,
                opacity: (i % 10) as f32 / 10.0,
            })
            .unwrap();
        }
        let mut steps = 0;
        while s.undo() {
            steps += 1;
        }
        assert_eq!(steps, MAX_UNDO);
        assert!(s.is_dirty());
        s.mark_saved();
        assert!(!s.is_dirty());
        s.redo();
        assert!(s.is_dirty());
    }

    #[test]
    fn a_group_of_commands_is_one_step_and_all_or_nothing() {
        let (mut s, a) = session();
        let b = s.project().tracks[0].clips[1].id;
        let outcomes = s
            .dispatch_all(vec![
                Command::SetOpacity {
                    clip: a,
                    opacity: 0.5,
                },
                Command::SetOpacity {
                    clip: b,
                    opacity: 0.4,
                },
            ])
            .unwrap();
        assert_eq!(outcomes.len(), 2);
        assert_eq!(s.project().clip(b).unwrap().1.opacity, 0.4);
        assert!(s.undo());
        assert_eq!(s.project().clip(a).unwrap().1.opacity, 1.0);
        assert_eq!(s.project().clip(b).unwrap().1.opacity, 1.0);
        assert!(!s.undo(), "the group was one step");

        // The second command fails: the first must not stay applied, and no step is added.
        let before = s.project().clone();
        let revision = s.revision();
        let failed = s.dispatch_all(vec![
            Command::SetOpacity {
                clip: a,
                opacity: 0.1,
            },
            Command::Delete { clip: ClipId(404) },
        ]);
        assert_eq!(failed, Err(EditError::NoSuchClip));
        assert_eq!(*s.project(), before);
        assert_eq!(s.revision(), revision);
        assert!(!s.undo());
        assert_eq!(s.dispatch_all(Vec::new()), Ok(Vec::new()));
    }
}
