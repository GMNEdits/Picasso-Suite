//! Project files (`.pcsp`): versioned JSON.

use crate::model::Project;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const EXTENSION: &str = "pcsp";
const FORMAT: &str = "picasso-suite-project";
const VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum ProjectIoError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid project file: {0}")]
    Json(#[from] serde_json::Error),
    #[error("not a Picasso Suite project")]
    WrongFormat,
    #[error("project version {0} is newer than this build supports ({VERSION})")]
    TooNew(u32),
}

#[derive(Serialize, Deserialize)]
struct ProjectFile {
    format: String,
    version: u32,
    project: Project,
}

pub fn save(project: &Project, path: &Path) -> Result<(), ProjectIoError> {
    let file = ProjectFile { format: FORMAT.into(), version: VERSION, project: project.clone() };
    let json = serde_json::to_vec_pretty(&file)?;
    // Write to a temp file first so a crash never leaves a half-written project.
    let tmp = path.with_extension(format!("{EXTENSION}.tmp"));
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn load(path: &Path) -> Result<Project, ProjectIoError> {
    let bytes = std::fs::read(path)?;
    let file: ProjectFile = serde_json::from_slice(&bytes)?;
    if file.format != FORMAT {
        return Err(ProjectIoError::WrongFormat);
    }
    if file.version > VERSION {
        return Err(ProjectIoError::TooNew(file.version));
    }
    let mut project = file.project;
    // Relative media paths (hand-written or portable projects) are relative to the project file.
    if let Some(dir) = path.parent() {
        for m in project.media.iter_mut().filter(|m| m.path.is_relative()) {
            m.path = dir.join(&m.path);
        }
    }
    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("pcsp-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.pcsp");
        let p = Project::new("Roundtrip", Default::default());
        save(&p, &path).unwrap();
        assert_eq!(load(&path).unwrap(), p);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn relative_media_resolves_against_project_dir() {
        let dir = std::env::temp_dir().join(format!("pcsp-rel-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.pcsp");
        let mut p = Project::default();
        p.media.push(crate::model::MediaItem {
            id: crate::model::new_id(),
            name: "a".into(),
            path: "media/a.mov".into(),
            sequence: None,
            info: Default::default(),
        });
        save(&p, &path).unwrap();
        assert_eq!(load(&path).unwrap().media[0].path, dir.join("media/a.mov"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
