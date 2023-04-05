/// Module for abstracting framework-specific logic.
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
    result::Result,
};
use walkdir::{DirEntry, WalkDir};

pub trait Framework {
    fn new(path: &Path) -> Result<Self, Box<dyn Error + Send + Sync>>
    where
        Self: Sized;
    fn is_supported(path: &Path) -> bool;
    fn build_commands(&self) -> Result<Vec<Command>, Box<dyn Error>>;
    fn get_artifacts(&self) -> Result<Vec<PathBuf>, Box<dyn Error>>;
}

pub struct Foundry {
    path: PathBuf,
}

impl Foundry {
    fn filter_artifacts(artifacts: Vec<PathBuf>) -> Vec<PathBuf> {
        // Filter out artifacts where all sources are in the `lib/` directory.
        artifacts
            .into_iter()
            .filter(|a| {
                let content = fs::read_to_string(a).unwrap();
                let json: serde_json::Value = serde_json::from_str(&content).unwrap();
                if let Some(metadata) = json.get("metadata") {
                    if let Some(sources) = metadata.get("sources") {
                        let sources_obj = sources.as_object().unwrap();
                        let all_sources_are_libs =
                            sources_obj.keys().all(|key| key.starts_with("lib/"));
                        return !all_sources_are_libs
                    }
                }
                false // If metadata and sources are missing, this can't be the right contract.
            })
            .collect::<Vec<_>>()
    }

    fn foundry_profiles(config_file: &PathBuf) -> Result<Vec<String>, Box<dyn Error>> {
        let contents = fs::read_to_string(config_file).unwrap();
        let data = contents.parse::<toml::Value>();
        if data.is_err() {
            return Err("Unable to parse foundry.toml file".into())
        }

        let mut profiles = Vec::new();
        if let Some(profiles_table) =
            data.unwrap().as_table().unwrap().get("profile").and_then(|v| v.as_table())
        {
            for key in profiles_table.keys() {
                profiles.push(key.to_string());
            }
        }

        if !profiles.contains(&"default".to_string()) {
            profiles.push("default".to_string());
        }
        Ok(profiles)
    }
}

impl Framework for Foundry {
    // Return an instance of the framework if the path is a supported project.
    fn new(path: &Path) -> Result<Self, Box<dyn Error + Send + Sync>> {
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

    // TODO We currently only support forge projects and assume the user is using the default forge
    // directory structure of `src/`, `lib/`, and `out/`.
    fn build_commands(&self) -> Result<Vec<Command>, Box<dyn Error>> {
        let config_file = self.path.join("foundry.toml");
        let profile_names = Self::foundry_profiles(&config_file)?;
        println!("  Found profiles: {:?}", profile_names);
        let profile_names = vec!["optimized".to_string()]; // TODO temporary seaport hack
        let commands = profile_names
            .into_iter()
            .map(|profile_name| {
                let mut command = Command::new("forge");
                command
                    .arg("build")
                    .arg("--skip")
                    .arg("test")
                    .arg("script")
                    .arg("--build-info")
                    .arg("--build-info-path")
                    .arg("build_info")
                    .env("FOUNDRY_PROFILE", profile_name)
                    .env("FOUNDRY_BYTECODE_HASH", "none"); // TODO Account for bytecode hash later.
                command
            })
            .collect::<Vec<Command>>();
        Ok(commands)
    }

    fn get_artifacts(&self) -> Result<Vec<PathBuf>, Box<dyn Error>> {
        let mut artifacts = Vec::new();

        fn is_out_dir(entry: &DirEntry) -> bool {
            entry.file_type().is_dir()
                && entry.file_name().to_string_lossy().to_lowercase().contains("out")
        }

        let out_dirs =
            WalkDir::new(&self.path).min_depth(1).max_depth(1).into_iter().filter_entry(is_out_dir);

        for entry in out_dirs {
            let entry = entry?;
            if entry.path().is_dir() {
                for inner_entry in WalkDir::new(entry.path()) {
                    let inner_entry = inner_entry?;
                    if inner_entry.file_type().is_file()
                        && inner_entry.path().extension().map_or(false, |ext| ext == "json")
                    {
                        artifacts.push(inner_entry.into_path());
                    }
                }
            }
        }

        Ok(Self::filter_artifacts(artifacts))
    }
}
