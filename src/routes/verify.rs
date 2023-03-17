use crate::provider::{contract_creation_data, provider_from_chain};
use axum::{http, response::IntoResponse, Json};
use ethers::types::{Chain, TxHash};
use git2::{Oid, Repository};
use serde::Deserialize;
use std::{error::Error, path::Path, str::FromStr};
use tempfile::TempDir;

#[derive(Deserialize)]
pub struct VerifyData {
    repo_url: String,
    repo_commit: String,
    contract_address: String,
    chain_id: u64,
    // TODO Remove this and find it.
    creation_tx_hash: String,
}

#[tracing::instrument(
    name = "Verifying contract",
    skip(json),
    fields(
        repo_url = %json.repo_url,
        repo_commit = %json.repo_commit,
        contract_address = %json.contract_address,
        creation_tx_hash = %json.creation_tx_hash,
    )
)]
pub async fn verify(Json(json): Json<VerifyData>) -> impl IntoResponse {
    let repo_url = json.repo_url.as_str();
    let commit_hash = json.repo_commit.as_str();
    let chain_id = Chain::try_from(json.chain_id).unwrap();
    // let chain_id =
    let tx_hash = TxHash::from_str(&json.creation_tx_hash).unwrap();

    let provider = provider_from_chain(chain_id);
    let creation_code = contract_creation_data(&provider, tx_hash).await;

    // Return an error if there's no creation code for the transaction hash.
    if creation_code.is_none() {
        return (
            http::StatusCode::BAD_REQUEST,
            format!("No creation code for tx hash {tx_hash} on chain ID {chain_id}"),
        )
    }

    // Create a temporary directory for the cloned repository.
    let temp_dir = TempDir::new().unwrap();

    // Clone the repository and checking out the commit.
    let possible_repo = clone_repo_and_checkout_commit(repo_url, commit_hash, &temp_dir).await;
    if possible_repo.is_err() {
        return (http::StatusCode::BAD_REQUEST, format!("Unable to clone repository {repo_url}"))
    }
    let repo = possible_repo.unwrap();

    // Verify this is a foundry project by looking for the presence of a foundry.toml file.
    // check if a file exists in the repository (using repo status_file method)
    let found_file = repo.status_file(Path::new("foundry.toml"));
    if found_file.is_err() {
        return (
            http::StatusCode::BAD_REQUEST,
            format!("No foundry.toml file found in repository {repo_url}"),
        )
    }

    // Extract profiles from the foundry.toml file.

    // Build the source directory for each profile.

    // Check if any of the creation codes match the bytecode of the contract. If so, we were
    // successful.

    // None match, so return an error with some info.
    (http::StatusCode::OK, "OK".to_string())
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
