use crate::{
    bytecode::MatchType,
    frameworks::{Foundry, Framework},
    provider::{ChainResponse, MultiChainProvider},
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
};
use tempfile::TempDir;

#[derive(Deserialize, Debug)]
pub enum BuildFramework {
    #[serde(rename = "foundry")]
    Foundry,
    #[serde(rename = "hardhat")]
    Hardhat,
    #[serde(rename = "ape")]
    Ape,
    #[serde(rename = "truffle")]
    Truffle,
}

#[derive(Deserialize)]
pub struct BuildConfig {
    framework: BuildFramework,
    // For forge, this is the profile name.
    build_hint: Option<String>,
}

#[derive(Deserialize)]
pub struct VerifyData {
    repo_url: String,
    repo_commit: String,
    contract_address: Address,
    build_config: BuildConfig,
    creation_tx_hashes: Option<HashMap<Chain, TxHash>>,
}

#[derive(Serialize, Deserialize)]
pub struct CompilerInfo {
    compiler: String, // Includes version.
    language: String,
    settings: MetadataSettings,
}

#[derive(Serialize, Deserialize)]
pub struct SuccessfulVerification {
    pub repo_url: String,
    pub repo_commit: String,
    pub contract_address: Address,
    pub matches: HashMap<Chain, VerificationMatch>,
    pub creation_tx_hash: Option<TxHash>,
    pub creation_block_number: Option<u64>,
    pub creation_code: Option<Bytes>,
    pub sources: Vec<SourceFile>,
    pub runtime_code: Bytes,
    pub creation_bytecode: Option<CompactBytecode>,
    pub deployed_bytecode: CompactDeployedBytecode,
    pub abi: LosslessAbi,
    pub compiler_info: CompilerInfo,
    pub ast: Ast,
}

#[derive(Serialize, Deserialize)]
pub struct SourceFile {
    path: PathBuf,
    content: String,
}

#[derive(Serialize, Deserialize)]
pub struct VerificationMatch {
    artifact: PathBuf,
    creation_code_match_type: MatchType,
    deployed_code_match_type: MatchType,
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

#[tracing::instrument(
    name = "Verifying contract",
    skip(json),
    fields(
        repo_url = %json.repo_url,
        repo_commit = %json.repo_commit,
        contract_address = %json.contract_address,
    )
)]
pub async fn verify(Json(json): Json<VerifyData>) -> Result<Response, VerifyError> {
    println!("\nVERIFICATION INPUTS:");
    println!("  Repo URL:         {}", json.repo_url);
    println!("  Commit Hash:      {}", json.repo_commit);
    println!("  Contract Address: {:#?}", json.contract_address);

    println!("\nVERIFYING INPUTS");
    let provider = MultiChainProvider::default();
    let temp_dir = TempDir::new()?;
    let project_path = &temp_dir.path();

    let deployed_code = verify_user_inputs(&json, project_path, &provider).await?;
    let creation_data =
        provider.get_creation_code(json.contract_address, json.creation_tx_hashes).await;

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
