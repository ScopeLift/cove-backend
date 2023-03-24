use crate::{compile, provider::MultiChainProvider};
use axum::{http, response::IntoResponse, Json};
use ethers::types::{Address, Chain};
use git2::{Oid, Repository};
use serde::Deserialize;
use std::{
    collections::HashMap,
    error::Error,
    path::{Path, PathBuf},
    str::FromStr,
};
use tempfile::TempDir;

#[derive(Deserialize)]
pub struct VerifyData {
    repo_url: String,
    repo_commit: String,
    contract_address: String,
}

#[tracing::instrument(
    name = "Verifying contract",
    skip(json),
    fields(
        repo_url = %json.repo_url,
        repo_commit = %json.repo_commit,
        contract_address = %json.contract_address,
    )
)]
pub async fn verify(Json(json): Json<VerifyData>) -> impl IntoResponse {
    let repo_url = json.repo_url.as_str();
    let commit_hash = json.repo_commit.as_str();
    let contract_addr = Address::from_str(json.contract_address.as_str()).unwrap();

    let provider = MultiChainProvider::default();
    let creation_data = provider.get_creation_code(contract_addr).await;

    // Return an error if there's no creation code for the transaction hash.
    if creation_data.is_all_none() {
        let msg = format!("No creation code for {:?} found on any supported chain", contract_addr);
        return (http::StatusCode::BAD_REQUEST, msg)
    }

    // Create a temporary directory for the cloned repository.
    let temp_dir = TempDir::new().unwrap();
    let path = &temp_dir.path();

    // Clone the repository and checking out the commit.
    let maybe_repo = clone_repo_and_checkout_commit(repo_url, commit_hash, &temp_dir).await;
    if maybe_repo.is_err() {
        return (http::StatusCode::BAD_REQUEST, format!("Unable to clone repository {repo_url}"))
    }

    // Get the build commands for the project.
    let build_commands = compile::build_commands(path).unwrap();
    let mut verified_contracts: HashMap<Chain, PathBuf> = HashMap::new();

    for mut build_command in build_commands {
        // Build the contracts.
        std::env::set_current_dir(path).unwrap();
        let build_result = build_command.output().unwrap();
        if !build_result.status.success() {
            continue // This profile might not compile, e.g. perhaps it fails with stack too deep.
        }

        let artifacts = compile::get_artifacts(Path::join(path, "out")).unwrap();
        let matches = provider.compare_creation_code(artifacts, &creation_data);

        // If two profiles match, we overwrite the first with the second. This is ok, because solc
        // inputs to outputs are not necessarily 1:1, e.g. changing optimization settings may not
        // change bytecode. This is likely true for other compilers too.
        for (chain, path) in matches.iter_entries() {
            verified_contracts.insert(*chain, path.clone());
        }
    }

    if verified_contracts.is_empty() {
        return (http::StatusCode::BAD_REQUEST, "No matching contracts found".to_string())
    }
    (http::StatusCode::OK, "Verified contract!".to_string())
}

async fn clone_repo_and_checkout_commit(
    repo_url: &str,
    commit_hash: &str,
    temp_dir: &TempDir,
) -> Result<Repository, Box<dyn Error>> {
    // Clone the repository.
    let repo = Repository::clone(repo_url, temp_dir.path())?;

    // Find the specified commit (object ID).
    let oid = Oid::from_str(commit_hash)?;
    let commit = repo.find_commit(oid)?;

    // Create a branch for the commit.
    let branch = repo.branch(commit_hash, &commit, false);

    // Checkout the commit.
    let obj = repo.revparse_single(&("refs/heads/".to_owned() + commit_hash)).unwrap();
    repo.checkout_tree(&obj, None)?;

    repo.set_head(&("refs/heads/".to_owned() + commit_hash))?;

    // Drop objects that have references to the repo so that we can return it.
    drop(branch);
    drop(commit);
    drop(obj);
    Ok(repo)
}
