/// Module for abstracting framework-specific logic.
use std::{
    error::Error,
    path::{Path, PathBuf},
    process::Command,
    result::Result,
};

pub struct Artifact;

pub trait Framework {
    fn new(path: &Path) -> Result<Self, Box<dyn Error>>
    where
        Self: Sized;
    fn is_supported(path: &Path) -> bool;
    fn build_commands(&self, path: &Path) -> Result<Vec<Command>, Box<dyn Error>>;
    fn get_artifacts(&self, path: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>>;
}

pub struct Foundry {
    path: PathBuf,
}

impl Framework for Foundry {
    // Return an instance of the framework if the path is a supported project.
    fn new(path: &Path) -> Result<Self, Box<dyn Error>> {
        if !Self::is_supported(path) {
            return Err("Not a foundry project.".into())
        }
        Ok(Self { path: path.to_path_buf() })
    }

    // Verify this is a foundry project by looking for the presence of a `foundry.toml` file.
    fn is_supported(path: &Path) -> bool {
        let file = path.join("foundry.toml");
        file.exists() && file.is_file()
    }

    fn build_commands(&self, path: &Path) -> Result<Vec<Command>, Box<dyn Error>> {
        let config_file = self.path.join("foundry.toml");
        Ok(Vec::new())
    }
    fn get_artifacts(&self, path: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
        Ok(Vec::new())
    }
}
