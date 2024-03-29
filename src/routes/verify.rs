use crate::{
    bytecode::MatchType,
    frameworks::{foundry::Foundry, framework::Framework},
    provider::{ChainResponse, MultiChainProvider},
};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use ethers::types::{Address, Bytes, Chain, TxHash};
use ethers_solc::{
    artifacts::{Ast, CompactBytecode, CompactDeployedBytecode, LosslessAbi, MetadataSettings},
    buildinfo::BuildInfo,
    ConfigurableContractArtifact,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
    result::Result,
    time::{SystemTime, UNIX_EPOCH},
};
use tempfile::TempDir;
use uuid::Uuid;

/// The build framework used by the repository.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum BuildFramework {
    /// [Foundry](https://book.getfoundry.sh/).
    Foundry,
    /// [Hardhat](https://hardhat.org/).
    Hardhat,
    /// [Ape](https://apeworx.io/).
    Ape,
    /// [Truffle](https://trufflesuite.com/).
    Truffle,
}

/// Data provided by the caller to instruct the Cove API how to build a repo.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BuildConfig {
    /// The build framework used by the project. Currently only Foundry is supported.
    framework: BuildFramework,
    /// A framework-specific build hint. For Foundry this is the name of the profile to build with.
    build_hint: Option<String>,
}

/// Data that a caller provides to verify a contract.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VerifyData {
    /// The URL of the repository.
    repo_url: String,
    /// The commit hash of the repository.
    repo_commit: String,
    /// The address of the contract to verify.
    contract_address: Address,
    /// The build configuration for the project, such as the framework and build instructions.
    build_config: BuildConfig,
    /// Optional, the transaction hashes that created the contract. For now these are required to
    /// verify creation code, to avoid binary searching blocks and tracing transactions to find
    /// creation code.
    creation_tx_hashes: Option<HashMap<Chain, TxHash>>,
}

/// Details about the compiler used to compile the contract.
#[derive(Serialize, Deserialize)]
pub struct CompilerInfo {
    /// The compiler name and version.
    compiler: String,
    /// The language name.
    language: String,
    /// The compiler metadata settings.
    settings: MetadataSettings,
}

/// Data returned for a successful verification.
#[derive(Serialize, Deserialize)]
pub struct SuccessfulVerification {
    /// The URL of the repository.
    pub repo_url: String,
    /// The commit hash of the repository.
    pub repo_commit: String,
    /// The address of the contract that was verified.
    pub contract_address: Address,
    /// A map from chain to the matching contract artifact and match types.
    pub matches: HashMap<Chain, VerificationMatch>,
    /// The transaction hash that created the contract.
    pub creation_tx_hash: Option<TxHash>,
    /// The block number containing the transaction hash that created the contract.
    pub creation_block_number: Option<u64>,
    /// The creation code of the contract, also known as the init code. This is the code that was
    /// executed to return the deployed code.
    pub creation_code: Option<Bytes>,
    /// Array of source files that were used to compile the contract. The first source file is the
    /// most-derived contract, i.e. the one that was deployed and verified.
    pub sources: Vec<SourceFile>,
    /// The deployed code of the contract, also known as the runtime code. This is the code that
    /// was returned by executing the creation code and lives at the contract address on-chain.
    pub runtime_code: Bytes,
    /// The creation code of the contract, also known as the init code. This is the code that was
    /// executed to return the deployed code.
    pub creation_bytecode: Option<CompactBytecode>,
    /// The deployed code of the contract, also known as the runtime code. This is the code that
    /// was returned by executing the creation code and lives at the contract address on-chain.
    pub deployed_bytecode: CompactDeployedBytecode,
    /// The ABI of the verified contract.
    pub abi: LosslessAbi,
    /// The name, version, and metadata settings of the compiler used to compile the contract.
    pub compiler_info: CompilerInfo,
    /// The abstract syntax tree of the verified contract.
    pub ast: Ast,
}

/// Data about a specific Solidity source file.
#[derive(Serialize, Deserialize)]
pub struct SourceFile {
    /// The path to the source file.
    path: PathBuf,
    /// The full content of the source file, including both code and comments.
    content: String,
}

/// Contains data about whether the `artifact` matches the expected creation code or deployed code.
#[derive(Serialize, Deserialize)]
pub struct VerificationMatch {
    /// The path to the artifact.
    artifact: PathBuf,
    /// The type of match for the creation code.
    creation_code_match_type: MatchType,
    /// The type of match for the deployed code.
    deployed_code_match_type: MatchType,
}

/// Fields in the Airtable database that are saved.
#[derive(Serialize)]
struct LogFields {
    /// The UUID of the record. Each database record has a unique UUID.
    #[serde(rename = "UUID")]
    uuid: String,
    /// The request ID generated by the API when called. This allows us to link two entries from
    /// the same HTTP request together, since we save request data twice: once immediately on
    /// entry, and again after verification has been attempted.
    #[serde(rename = "Request ID")]
    request_id: String,
    /// The timestamp of the request.
    #[serde(rename = "Timestamp")]
    timestamp: u128,
    /// The URL of the repository.
    #[serde(rename = "Repo URL")]
    repo_url: String,
    /// The commit hash of the repository.
    #[serde(rename = "Commit Hash")]
    commit_hash: String,
    /// The address of the contract to verify.
    #[serde(rename = "Contract Address")]
    contract_address: String,
    /// The chain IDs that the caller wants to verify the contract on.
    #[serde(rename = "Chain IDs")]
    chain_ids: String,
    /// Whether the verification was successful.
    #[serde(rename = "Success")]
    success: String,
}

/// Data sent to the Airtable API to save request data.
#[derive(Serialize)]
struct LogRecord {
    /// The fields in the Airtable database that are saved.
    fields: LogFields,
}

/// Returned if verification failed.
pub enum VerifyError {
    /// The caller provided data that was invalid.
    BadRequest(String),
    /// The server encountered an error that was not the caller's fault.
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

/// Automatically implements the `From` trait for the provided error, to simplify the process of
/// converting various error types into `VerifyError`.
///
/// # Arguments
///
/// * `$error_type:ty` - The specific error type you want to convert into `VerifyError`.
macro_rules! impl_from_for_verify_error {
    ($error_type:ty) => {
        impl From<$error_type> for VerifyError {
            fn from(err: $error_type) -> Self {
                VerifyError::InternalServerError(err.to_string())
            }
        }
    };
}

impl_from_for_verify_error!(Box<dyn std::error::Error>);
impl_from_for_verify_error!(std::io::Error);
impl_from_for_verify_error!(&str);
impl_from_for_verify_error!(serde_json::Error);

// ===================================
// ======== Main verification ========
// ===================================

/// Entrypoint for contract verification.
///
/// # Arguments
///
/// * `json` - The JSON payload containing verification input data.
///
/// # Returns
///
/// Returns a `Result` containing a `Response` if verification was successful, or a `VerifyError` if
/// verification failed.
#[tracing::instrument(
    name = "Verifying contract",
    skip(json),
    fields(
        request_id = %Uuid::new_v4(),
        repo_url = %json.repo_url,
        repo_commit = %json.repo_commit,
        contract_address = ?json.contract_address,
        creation_tx_hashes = ?json.creation_tx_hashes,
    )
)]
pub async fn verify(Json(json): Json<VerifyData>) -> Result<Response, VerifyError> {
    println!("\nVERIFICATION INPUTS:");
    println!("  Repo URL:         {}", json.repo_url);
    println!("  Commit Hash:      {}", json.repo_commit);
    println!("  Contract Address: {:#?}", json.contract_address);

    println!("\nSAVING INPUTS");
    // For simplicity for now, we generate a new UUID here since the `tracing::instrument` request
    // ID is not available here.
    let request_id = Uuid::new_v4();
    let _ = save_data(
        Uuid::new_v4(),
        request_id,
        &json.repo_url,
        &json.repo_commit,
        &json.contract_address,
        &json.creation_tx_hashes,
        false,
    )
    .await;

    println!("\nVERIFYING INPUTS");
    let provider = MultiChainProvider::default();
    let temp_dir = TempDir::new()?;
    let project_path = &temp_dir.path();

    let deployed_code = verify_user_inputs(&json, project_path, &provider).await?;
    let creation_data =
        provider.get_creation_code(json.contract_address, json.creation_tx_hashes.clone()).await;

    // Determine the framework used by the project. For now we only support Foundry.
    let project = match json.build_config.framework {
        BuildFramework::Foundry => Foundry::new(project_path).map_err(|e| {
            VerifyError::BadRequest(format!("Failed to create Foundry project: {}", e))
        })?,
        _ => {
            let msg = format!("Unsupported framework: {:?}", json.build_config.framework);
            return Err(VerifyError::BadRequest(msg))
        }
    };

    // Get the build commands for the project.
    println!("\nBUILDING CONTRACTS AND COMPARING BYTECODE");
    std::env::set_current_dir(project_path)?;
    let build_commands = project.build_commands(json.build_config.build_hint)?;
    let mut verified_contracts: HashMap<Chain, VerificationMatch> = HashMap::new();

    for mut build_command in build_commands {
        println!("  Building with command: {}", format!("{:?}", build_command).replace('"', ""));

        // Build the contracts.
        let build_result = build_command.output()?;
        if !build_result.status.success() {
            println!("    Build failed, continuing to next build command.");
            continue // This profile might not compile, e.g. perhaps it fails with stack too deep.
        }
        println!("    Build succeeded, comparing creation code.");

        let deployed_matches = provider.compare_deployed_code(&project, &deployed_code);
        let creation_matches = match &creation_data {
            Ok(creation_data) => provider.compare_creation_code(&project, creation_data),
            Err(_) => ChainResponse::default(),
        };

        if deployed_matches.is_all_none() && creation_matches.is_all_none() {
            println!("    No matching contracts found, continuing to next build command.");
        }

        // We found matches, so save them off.
        // If two profiles match, we overwrite the first with the second. This is ok, because solc
        // inputs to outputs are not necessarily 1:1, e.g. changing optimization settings may not
        // change bytecode. This is likely true for other compilers too.
        for chain in &provider.chains {
            let deployed_match = deployed_matches.responses.get(chain).cloned().flatten();
            let creation_match = creation_matches.responses.get(chain).cloned().flatten();
            match (deployed_match, creation_match) {
                (Some(deployed_match), Some(creation_match)) => {
                    if deployed_match.artifact != creation_match.artifact {
                        println!("    ❌ Found conflicting matches on chain {:?}:", chain);
                        println!("        Creation: {:?}", creation_match.artifact);
                        println!("        Deployed: {:?}", deployed_match.artifact);
                        println!("        Continuing to next build command.");
                        continue
                    }
                    // Extract contract name from path by removing the extension
                    let stem = deployed_match.artifact.file_stem().ok_or("Bad file name")?;
                    println!(
                        "    ✅ Found matching creation and deployed code on chain {:?}: {:?}",
                        chain, stem
                    );

                    // Save off the match.
                    let verification_match = VerificationMatch {
                        artifact: creation_match.artifact,
                        creation_code_match_type: creation_match.match_type,
                        deployed_code_match_type: deployed_match.match_type,
                    };
                    verified_contracts.insert(*chain, verification_match);
                }
                (Some(deployed_match), None) => {
                    let stem = deployed_match.artifact.file_stem().ok_or("Bad file name")?;
                    println!(
                        "    ✅ Found matching deployed code on chain {:?}: {:?}",
                        chain, stem
                    );

                    // Save off the match.
                    let verification_match = VerificationMatch {
                        artifact: deployed_match.artifact,
                        creation_code_match_type: MatchType::None,
                        deployed_code_match_type: deployed_match.match_type,
                    };
                    verified_contracts.insert(*chain, verification_match);
                }
                (None, Some(creation_match)) => {
                    let stem = creation_match.artifact.file_stem().ok_or("Bad file name")?;
                    println!(
                        "    ✅ Found matching creation code on chain {:?}: {:?}",
                        chain, stem
                    );

                    // Save off the match.
                    let verification_match = VerificationMatch {
                        artifact: creation_match.artifact,
                        creation_code_match_type: creation_match.match_type,
                        deployed_code_match_type: MatchType::None,
                    };
                    verified_contracts.insert(*chain, verification_match);
                }
                (None, None) => {}
            }
        }
    }

    if verified_contracts.is_empty() {
        return Ok(
            (StatusCode::BAD_REQUEST, "No matching contracts found".to_string()).into_response()
        )
    }
    println!("\nCONTRACT VERIFICATION SUCCESSFUL!");
    println!("\nPREPARING RESPONSE");

    // ======== Format Response ========
    // Format response. If there are multiple chains we verified on, we just return an arbitrary one
    // for now. For now we just hardcode Optimism for demo purposes.

    // Get the artifact for the contract. We just arbitrarily pick the first one.
    let chain = &verified_contracts.keys().next().unwrap().clone();
    let contract_match = verified_contracts.get(chain).unwrap();
    let artifact_content = fs::read_to_string(&contract_match.artifact)?;
    let artifact: ConfigurableContractArtifact = serde_json::from_str(&artifact_content)?;

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
        .ok_or("Bad file name")?;

    let build_info_content = fs::read_to_string(build_info_file.path())?;

    // Now we merge the data into our sources vector.
    let build_info: BuildInfo = serde_json::from_str(&build_info_content)?;

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
    let block_num = creation_data
        .as_ref()
        .ok()
        .and_then(|data| data.responses.get(chain))
        .and_then(|resp| resp.as_ref())
        .map(|resp| resp.block);
    let selected_creation_data = creation_data
        .as_ref()
        .ok()
        .and_then(|data| data.responses.get(chain))
        .and_then(|resp| resp.as_ref());

    // Assemble and return the response.
    let creation_tx_hash = selected_creation_data.map(|x| x.tx_hash);
    let creation_block_number = block_num.map(|x| x.as_number().unwrap().as_u64());
    let creation_code = selected_creation_data.map(|x| x.creation_code.clone());

    let _ = save_data(
        Uuid::new_v4(),
        request_id,
        &json.repo_url,
        &json.repo_commit,
        &json.contract_address,
        &json.creation_tx_hashes,
        true,
    )
    .await;

    let response = SuccessfulVerification {
        repo_url: json.repo_url,
        repo_commit: json.repo_commit,
        contract_address: json.contract_address,
        matches: verified_contracts,
        sources,
        creation_tx_hash,
        creation_block_number,
        creation_code,
        runtime_code: deployed_code.responses.get(chain).unwrap().clone().unwrap(),
        creation_bytecode: Some(artifact.bytecode.unwrap()),
        deployed_bytecode: artifact.deployed_bytecode.unwrap(),
        abi: artifact.abi.unwrap(),
        compiler_info,
        ast: artifact.ast.unwrap(),
    };

    println!("\nFINISHED");
    println!("  200 response returned.");

    Ok((StatusCode::OK, Json(response)).into_response())
}

/// Takes the user inputs and uses the multichain provider to ensure inputs are valid:
///   - It clones the repo, which might fail if the repo is private or doesn't exist.
///   - It ensures there is code at the given contract address on at least 1 chain.
async fn verify_user_inputs(
    json: &VerifyData,
    project_path: &Path,
    provider: &MultiChainProvider,
) -> Result<ChainResponse<Bytes>, VerifyError> {
    // Clone repo and checkout commit
    match clone_repo_and_checkout_commit(&json.repo_url, &json.repo_commit, project_path).await {
        Ok(_) => (),
        Err(err) => {
            let msg = format!("Failed to clone repository or checkout commit: {}", err);
            return Err(VerifyError::BadRequest(msg))
        }
    };

    // Fetch deployed code
    let deployed_code = provider.get_deployed_code(json.contract_address).await?;
    if deployed_code.is_all_none() {
        return Err(VerifyError::BadRequest("No deployed code found for contract".to_string()))
    }

    Ok(deployed_code)
}

/// Clones the given repository to `temp_dir` and checks out the specified commit.
async fn clone_repo_and_checkout_commit(
    repo_url: &str,
    commit_hash: &str,
    temp_dir: &Path,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("  Cloning repository into a temporary directory.");

    let status = Command::new("git")
        .arg("clone")
        .arg(repo_url)
        .arg(".") // Clone directly into the `temp_dir` instead of creating a subdirectory.
        .arg("--quiet")
        .current_dir(temp_dir)
        .status()?;

    if !status.success() {
        return Err(format!("Failed to clone the repository. Exit status: {}", status).into())
    }

    println!("  Checking out the given commit.");
    let status = Command::new("git")
        .arg("checkout")
        .arg(commit_hash)
        .arg("--quiet")
        .current_dir(temp_dir)
        .status()?;

    if !status.success() {
        return Err(format!("Failed to checkout the commit. Exit status: {}", status).into())
    }
    println!("  Done.");
    Ok(())
}

/// Saves off request data to Airtable. This function runs twice: once immediately on entry, and
/// again after verification has been attempted.
async fn save_data(
    uuid: Uuid,
    request_id: Uuid,
    repo_url: &str,
    commit_hash: &str,
    contract_address: &Address,
    creation_tx_hashes: &Option<HashMap<Chain, TxHash>>,
    success: bool,
) {
    let client = reqwest::Client::new();

    let base_id = std::env::var("AIRTABLE_BASE_ID").unwrap_or_default();
    let table_id = std::env::var("AIRTABLE_TABLE_ID").unwrap_or_default();
    let pat = std::env::var("AIRTABLE_PAT").unwrap_or_default();

    // If all required environment variables are defined
    if !base_id.is_empty() && !table_id.is_empty() && !pat.is_empty() {
        let url = format!("https://api.airtable.com/v0/{base_id}/{table_id}");
        let chain_ids: String = match creation_tx_hashes {
            Some(map) => map
                .keys()
                .map(|chain| format!("{:?}", chain)) // Use format to convert Chain to String
                .collect::<Vec<_>>() // Collect the Strings into a Vec
                .join(","), // Join the Vec into a single String
            None => String::new(), // If there's no HashMap, use an empty String
        };

        let record = LogRecord {
            fields: LogFields {
                uuid: uuid.to_string(),
                request_id: request_id.to_string(),
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(),
                repo_url: repo_url.into(),
                commit_hash: commit_hash.into(),
                contract_address: format!("{:#?}", contract_address),
                chain_ids,
                success: if success { "true" } else { "N/A" }.into(),
            },
        };

        let _ = client.post(&url).bearer_auth(pat).json(&record).send().await;
    } else {
        println!("Env vars not defined, not saving off data.");
    }
}
