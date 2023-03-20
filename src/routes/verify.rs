use crate::{
    compile,
    provider::{contract_creation_data, provider_from_chain},
};
use axum::{http, response::IntoResponse, Json};
use ethers::types::{Chain, TxHash};
use git2::{Oid, Repository};
use serde::Deserialize;
use std::{error::Error, str::FromStr};
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
    let maybe_repo = clone_repo_and_checkout_commit(repo_url, commit_hash, &temp_dir).await;
    if maybe_repo.is_err() {
        return (http::StatusCode::BAD_REQUEST, format!("Unable to clone repository {repo_url}"))
    }

    // Build the project and get the resulting output.
    // TODO Some refactoring, ended up just doing the verification in the compile module for now.
    let verified_artifact = compile::compile(&temp_dir.path(), creation_code.unwrap());
    if verified_artifact.is_err() {
        return (http::StatusCode::BAD_REQUEST, verified_artifact.err().unwrap().to_string())
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
