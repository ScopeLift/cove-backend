use ethers::types::Bytes;
use serde_json;
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};
use toml;
use walkdir::WalkDir;

// TODO We currently assume the user is using the default forge directory structure of `src/`,
// `lib/`, and `out/`.
pub fn compile<P: AsRef<Path>>(
    path: &P,
    expected_creation_code: Bytes,
) -> Result<PathBuf, Box<dyn Error>> {
    if !is_foundry_project(path) {
        return Err("Currently only non-monorepo foundry projects are supported.".into())
    }

    let config_file = path.as_ref().join("foundry.toml");
    let maybe_profile_names = foundry_profiles(&config_file);
    if maybe_profile_names.is_err() {
        return Err(maybe_profile_names.err().unwrap())
    }
    let profile_names = maybe_profile_names.unwrap();

    // Save off the current path then `cd` into the forge project.
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(path)?;

    // Run `FOUNDRY_PROFILE=profile_name forge build --skip test` in the given directory.
    for profile_name in profile_names {
        let result = Command::new("forge")
            .arg("build")
            .arg("--skip")
            .arg("test")
            .env("FOUNDRY_PROFILE", profile_name)
            .output()?;
        if !result.status.success() {
            continue // This profile might not compile, e.g. perhaps it exits with stack too deep.
        }

        // At this point we've compiled the code successfully, so we can compare bytecode.
        let all_artifacts = get_artifacts(Path::join(path.as_ref(), "out"))?;
        let artifacts = all_artifacts
            .iter()
            .filter(|a| {
                // Filter out artifacts where all sources are in the lib/ directory.
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
            .collect::<Vec<_>>();

        // For each artifact, check if the bytecode matches the expected creation code.
        for artifact in artifacts {
            let content = fs::read_to_string(artifact).unwrap();
            let json: serde_json::Value = serde_json::from_str(&content).unwrap();
            if let Some(bytecode_value) = json.get("bytecode").unwrap().get("object") {
                if let Some(bytecode_str) = bytecode_value.as_str() {
                    let bytecode = Bytes::from_str(bytecode_str).unwrap();
                    // TODO This check won't always work, e.g. constructor args, metadata hash, etc.
                    if bytecode == expected_creation_code {
                        std::env::set_current_dir(original_dir)?;
                        return Ok(artifact.to_path_buf())
                    }
                }
            }
        }
    }

    // TODO More info about what went wrong.
    Err("Unable to find a matching artifact.".into())
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

fn get_artifacts<P: AsRef<Path>>(out_dir: P) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut artifacts = Vec::new();

    for entry in WalkDir::new(out_dir) {
        let entry = entry?;
        if entry.file_type().is_file()
            && entry.path().extension().map_or(false, |ext| ext == "json")
        {
            artifacts.push(entry.path().to_path_buf());
        }
    }

    Ok(artifacts)
}
