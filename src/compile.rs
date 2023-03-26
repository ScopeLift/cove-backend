use serde_json;
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use toml;
use walkdir::WalkDir;

// TODO We currently only support forge projects and assume the user is using the default forge
// directory structure of `src/`, `lib/`, and `out/`.
pub fn build_commands<P: AsRef<Path>>(path: &P) -> Result<Vec<Command>, Box<dyn Error>> {
    if !is_foundry_project(path) {
        return Err("Currently only non-monorepo foundry projects are supported.".into())
    }
    println!("  Detected forge project.");

    let config_file = path.as_ref().join("foundry.toml");
    let profile_names = foundry_profiles(&config_file)?;
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
                .env("FOUNDRY_PROFILE", profile_name);
            command
        })
        .collect::<Vec<Command>>();
    Ok(commands)
}

pub fn get_artifacts<P: AsRef<Path>>(out_dir: P) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut artifacts = Vec::new();

    for entry in WalkDir::new(out_dir) {
        let entry = entry?;
        if entry.file_type().is_file()
            && entry.path().extension().map_or(false, |ext| ext == "json")
        {
            artifacts.push(entry.path().to_path_buf());
        }
    }

    Ok(filter_artifacts(artifacts))
}

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

fn is_foundry_project<P: AsRef<Path>>(path: &P) -> bool {
    // Verify this is a foundry project by looking for the presence of a foundry.toml file.
    // check if a file exists in the repository (using repo status_file method)
    let file = path.as_ref().join("foundry.toml");
    file.exists() && file.is_file()
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
