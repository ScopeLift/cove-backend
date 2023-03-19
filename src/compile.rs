use ethers::types::Bytes;
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use toml::Value;
use walkdir::WalkDir;

// TODO We currently assume the user is using the default forge directory structure of `src/`,
// `lib/`, and `out/`.
pub fn compile<P: AsRef<Path>>(
    path: &P,
    expected_creation_code: Bytes,
) -> Result<(), Box<dyn Error>> {
    if !is_foundry_project(path) {
        return Err("Currently only non-monorepo foundry projects are supported.".into())
    }

    let config_file = path.as_ref().join("foundry.toml");
    let maybe_profile_names = foundry_profiles(&config_file);
    if maybe_profile_names.is_err() {
        return Err(maybe_profile_names.err().unwrap())
    }
    let profile_names = maybe_profile_names.unwrap();

    let mut found_match = false;
    for profile_name in profile_names {
        // Run `FOUNDRY_PROFILE=profile_name forge build --skip test` in the given directory.
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

        let all_artifacts = get_artifacts(path)?;
        let artifacts = all_artifacts
            .iter()
            .filter(|a| {
                // TODO Filter out artifacts that do not involve src contracts.
                true
            })
            .collect::<Vec<_>>();
    }

    Ok(())
}

fn is_foundry_project<P: AsRef<Path>>(path: &P) -> bool {
    // Verify this is a foundry project by looking for the presence of a foundry.toml file.
    // check if a file exists in the repository (using repo status_file method)
    let file = path.as_ref().join("foundry.toml");
    file.exists() && file.is_file()
}

fn foundry_profiles(config_file: &PathBuf) -> Result<Vec<String>, Box<dyn Error>> {
    let contents = fs::read_to_string(config_file).unwrap();
    let data = contents.parse::<Value>();
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
