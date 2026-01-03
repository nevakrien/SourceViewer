use directories::ProjectDirs;
use serde::Deserialize;
use std::{error::Error, fs, path::PathBuf};
use tui::layout::Constraint;

fn get_project_dir() -> Option<ProjectDirs> {
    ProjectDirs::from("", "", "source-viewer")
}

pub fn get_walk_config_path()->Option<PathBuf>{
	let proj = get_project_dir()?;
    Some(proj.preference_dir().join("walk-config.toml"))
}

#[derive(Debug, Deserialize, Default)]
pub struct WalkConfig {
    pub asm_precent: Option<u32>,
}

impl WalkConfig {
	pub fn get_layout(&self) -> Result<[Constraint; 2], Box<dyn Error>> {
	    let p = self.asm_precent.unwrap_or(53);
	    let left = 100u32
            .checked_sub(p)
            .ok_or_else(|| {
                format!("asm_percent must be ≤ 100 (got {})", p)
            })?;

        Ok([
            Constraint::Ratio(left, 100),
            Constraint::Ratio(p, 100),
        ])
	}
    pub fn get_global() -> Result<Self, Box<dyn Error>> {
        let Some(path) = get_walk_config_path() else {
            // No home dir / config dir available → act like no config
            return Ok(Self::default());
        };

        // Case 1: file does not exist → defaults (all None)
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(e) => {
                // Case 3: real IO error
                return Err(Box::new(e));
            }
        };

        // Case 2 / 3: parse
        let cfg: WalkConfig = toml::from_slice(&bytes)?;
        Ok(cfg)
    }
}
