// TODO remove this, it's due to autogenerated types from https://transform.tools/json-to-rust-serde.
use crate::{
    frameworks::{Foundry, Framework},
    provider::{contract_runtime_code, MultiChainProvider},
};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use ethers::{
    solc::{
        artifacts::{Ast, CompactBytecode, CompactDeployedBytecode, LosslessAbi, MetadataSettings},
        buildinfo::BuildInfo,
        ConfigurableContractArtifact,
    },
    types::{Address, Bytes, Chain, TxHash},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
    result::Result,
    str::FromStr,
};
use tempfile::TempDir;

#[derive(Deserialize)]
pub struct VerifyData {
    repo_url: String,
    repo_commit: String,
    contract_address: String,
}

#[derive(Serialize)]
pub struct CompilerInfo {
    compiler: String, // Includes version.
    language: String,
    settings: MetadataSettings,
}

#[derive(Serialize)]
struct SuccessfulVerification {
    repo_url: String,
    repo_commit: String,
    contract_address: Address,
    chains: Vec<Chain>, // All chains this address has verified code on.
    chain: Chain,       // The chain data is being returned for.
    creation_tx_hash: TxHash,
    creation_block_number: u64,
    creation_code: Bytes,
    sources: Vec<SourceFile>,
    runtime_code: Bytes,
    creation_bytecode: CompactBytecode,
    deployed_bytecode: CompactDeployedBytecode,
    abi: LosslessAbi,
    compiler_info: CompilerInfo,
    ast: Ast,
}

#[derive(Serialize)]
struct SourceFile {
    path: PathBuf,
    content: String,
}

pub enum VerifyError {
    BadRequest(String),
    InternalServerError(String),
}

impl IntoResponse for VerifyError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            VerifyError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            VerifyError::InternalServerError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        (status, error_message).into_response()
    }
}

// ===================================
// ======== Main verification ========
// ===================================

// #[tracing::instrument(
//     name = "Verifying contract",
//     skip(json),
//     fields(
//         repo_url = %json.repo_url,
//         repo_commit = %json.repo_commit,
//         contract_address = %json.contract_address,
//     )
// )]
pub async fn verify(Json(json): Json<VerifyData>) -> Result<Response, VerifyError> {
    let repo_url = json.repo_url.as_str();
    let commit_hash = json.repo_commit.as_str();
    let contract_addr = Address::from_str(json.contract_address.as_str())
        .map_err(|e| VerifyError::BadRequest(format!("Invalid contract address: {}", e)))?;

    println!("\nVERIFICATION INPUTS:");
    println!("  Repo URL:         {}", repo_url);
    println!("  Commit Hash:      {}", commit_hash);
    println!("  Contract Address: {:?}", contract_addr);

    println!("\nFETCHING CREATION CODE");
    let provider = MultiChainProvider::default();
    let creation_data = provider
        .get_creation_code(contract_addr)
        .await
        .map_err(|e| VerifyError::BadRequest(format!("Could not fetch creation code: {}", e)))?;
    println!("  Found creation code on the following chains: {:?}", creation_data.responses.keys());

    // Create a temporary directory for the cloned repository.
    let temp_dir = TempDir::new().map_err(|e| {
        VerifyError::InternalServerError(format!("Could not create directory to clone repo: {}", e))
    })?;
    let project_path = &temp_dir.path();

    // Clone the repository and check out the commit.
    println!("\nCLONING REPOSITORY");
    clone_repo_and_checkout_commit(repo_url, commit_hash, project_path)
        .await
        .map_err(|e| VerifyError::BadRequest(format!("Could not clone repo: {}", e)))?;

    // Determine the framework used by the project. For now we only support Foundry.
    let project = Foundry::new(project_path)
        .map_err(|e| VerifyError::BadRequest(format!("Only supports forge projects: {}", e)))?;

    // Get the build commands for the project.
    println!("\nBUILDING CONTRACTS AND COMPARING BYTECODE");
    let build_commands = project.build_commands().map_err(|e| {
        VerifyError::InternalServerError(format!("Could not find build commands: {}", e))
    })?;
    let mut verified_contracts: HashMap<Chain, PathBuf> = HashMap::new();

    for mut build_command in build_commands {
        println!("  Building with command: {}", format!("{:?}", build_command).replace('"', ""));

        // Build the contracts.
        std::env::set_current_dir(project_path).map_err(|e| {
            VerifyError::InternalServerError(format!("Could not set current directory: {}", e))
        })?;
        let build_result = build_command.output().map_err(|e| {
            VerifyError::InternalServerError(format!("Failed to execute command: {}", e))
        })?;
        if !build_result.status.success() {
            println!("    Build failed, continuing to next build command.");
            continue // This profile might not compile, e.g. perhaps it fails with stack too deep.
        }
        println!("    Build succeeded, comparing creation code.");

        let matches = provider.compare_creation_code(&project, &creation_data);

        if matches.is_all_none() {
            println!("    No matching contracts found, continuing to next build command.");
        }

        // If two profiles match, we overwrite the first with the second. This is ok, because solc
        // inputs to outputs are not necessarily 1:1, e.g. changing optimization settings may not
        // change bytecode. This is likely true for other compilers too.
        for (chain, path) in matches.iter_entries() {
            // Extract contract name from path by removing the extension
            let stem = path.file_stem().ok_or("Bad file name").map_err(|e| {
                VerifyError::InternalServerError(format!("Could not split file name: {}", e))
            })?;
            println!("    ✅ Found matching contract on chain {:?}: {:?}", chain, stem);
            verified_contracts.insert(*chain, path.clone());
        }
    }

    if verified_contracts.is_empty() {
        return Ok(
            (StatusCode::BAD_REQUEST, "No matching contracts found".to_string()).into_response()
        )
    }

    // If multiple matches found, tell user we are choosing one.
    if verified_contracts.len() > 1 {
        println!("\nCONTRACT VERIFICATION SUCCESSFUL!");
        println!("\nPREPARING RESPONSE");
        println!("  Multiple matching contracts found, choosing Optimism arbitrarily.");
    }

    // ======== Format Response ========
    // Format response. If there are multiple chains we verified on, we just return an arbitrary one
    // for now. For now we just hardcode Optimism for demo purposes.

    // Get the artifact for the contract.
    let artifact_path = verified_contracts.get(&Chain::Optimism).unwrap();
    let artifact_content = fs::read_to_string(artifact_path)
        .map_err(|e| VerifyError::InternalServerError(format!("Could not read artifact: {}", e)))?;
    let artifact: ConfigurableContractArtifact =
        serde_json::from_str(&artifact_content).map_err(|e| {
            VerifyError::InternalServerError(format!("Could not parse artifact: {}", e))
        })?;

    // Extract the compiler data.
    let metadata = artifact.metadata.unwrap();
    let compiler_info = CompilerInfo {
        compiler: metadata.compiler.version,
        language: metadata.language,
        settings: metadata.settings.clone(),
    };

    //  -------- Assemble the source code --------
    // First we get the path of the most-derived contract, i.e. the one that was verified that we
    // want first in the vector.
    let first_contract_path = metadata.settings.compilation_target.keys().next().unwrap();

    // Since the key names will always differ, we read them into a hash map.
    let source_file_names: Vec<String> = metadata.sources.inner.keys().cloned().collect();

    // Next we read the build info file which has all the source code already stringified. We don't
    // know the name of this file (since it's a hash), but it's the only file in the directory.
    let build_info_dir = temp_dir.path().join("build_info");
    let build_info_file = fs::read_dir(build_info_dir)
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.path().extension().unwrap_or_default() == "json")
        .ok_or("Bad file name")
        .map_err(|e| {
            VerifyError::InternalServerError(format!(
                "JSON file not found in build_info directory: {}",
                e
            ))
        })?;

    let build_info_content = fs::read_to_string(build_info_file.path()).map_err(|e| {
        VerifyError::InternalServerError(format!("Could not read build info file: {}", e))
    })?;

    // Now we merge the data into our sources vector.
    let build_info: BuildInfo = serde_json::from_str(&build_info_content).map_err(|e| {
        VerifyError::InternalServerError(format!("Could not parse build info file: {}", e))
    })?;

    let mut sources: Vec<SourceFile> = source_file_names
        .iter()
        .filter_map(|path| {
            let path = PathBuf::from(path);
            build_info
                .input
                .sources
                .get(&path)
                .map(|source_info| SourceFile { path, content: source_info.content.to_string() })
        })
        .collect();

    // Lastly, we put the root source file first.
    sources.sort_by(|a, b| {
        if a.path == PathBuf::from(first_contract_path) {
            std::cmp::Ordering::Less
        } else if b.path == PathBuf::from(first_contract_path) {
            std::cmp::Ordering::Greater
        } else {
            a.path.cmp(&b.path)
        }
    });

    // Get the creation data.
    let block_num = creation_data.responses.get(&Chain::Optimism).unwrap().as_ref().unwrap().block;
    let selected_creation_data =
        creation_data.responses.get(&Chain::Optimism).unwrap().as_ref().unwrap();

    // Assemble and return the response.
    let response = SuccessfulVerification {
        repo_url: repo_url.to_string(),
        repo_commit: commit_hash.to_string(),
        contract_address: contract_addr,
        chains: verified_contracts.keys().copied().collect(),
        chain: Chain::Optimism, // TODO Un-hardcode this
        sources,
        creation_tx_hash: selected_creation_data.tx_hash,
        creation_block_number: block_num.as_number().unwrap().as_u64(),
        creation_code: selected_creation_data.creation_code.clone(),
        runtime_code: contract_runtime_code(
            provider.providers.get(&Chain::Optimism).unwrap(),
            contract_addr,
        )
        .await,
        creation_bytecode: artifact.bytecode.unwrap(),
        deployed_bytecode: artifact.deployed_bytecode.unwrap(),
        abi: artifact.abi.unwrap(),
        compiler_info,
        ast: artifact.ast.unwrap(),
    };

    println!("\nFINISHED");
    println!("  200 response returned.");

    Ok((StatusCode::OK, Json(response)).into_response())
}

async fn clone_repo_and_checkout_commit(
    repo_url: &str,
    commit_hash: &str,
    temp_dir: &Path,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("  Cloning repository into a temporary directory.");
    let status =
        Command::new("git").arg("clone").arg(repo_url).arg(temp_dir).arg("--quiet").status()?;

    if !status.success() {
        return Err(format!("Failed to clone the repository. Exit status: {}", status).into())
    }

    let cwd = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir)?;

    println!("  Checking out the given commit.");
    let status = Command::new("git").arg("checkout").arg(commit_hash).arg("--quiet").status()?;

    if !status.success() {
        return Err(format!("Failed to checkout the commit. Exit status: {}", status).into())
    }

    std::env::set_current_dir(cwd)?;
    println!("  Done.");

    Ok(())
}
