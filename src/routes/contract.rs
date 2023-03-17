use crate::provider::{contract_runtime_code, provider_from_chain, provider_url_from_chain};
use axum::{
    extract::Query,
    http,
    response::{IntoResponse, Response},
    Json,
};
use ethers::types::{Address, Bytes, Chain};
use heimdall::decompile::DecompileBuilder;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tempfile::TempDir;

#[derive(Deserialize, Debug)]
pub struct ContractQuery {
    chain_id: u64,
    address: String,
}

#[derive(Serialize)]
pub struct VerifiedResponse {
    // TODO
}

#[derive(Serialize)]
pub struct DecompiledResponse {
    chain_id: Chain,
    address: Address,
    verified: bool,
    abi: String, // TODO stronger type from ethers-rs?
    disassembled: String,
    solidity: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    error: String,
}

enum ApiResponse {
    Success(DecompiledResponse),
    Error(ErrorResponse),
}

impl IntoResponse for ApiResponse {
    fn into_response(self) -> Response {
        match self {
            ApiResponse::Success(success) => (http::StatusCode::OK, Json(success)).into_response(),
            ApiResponse::Error(error) => {
                (http::StatusCode::BAD_REQUEST, Json(error)).into_response()
            }
        }
    }
}

// #[tracing::instrument(name = "Fetching contract")]
pub async fn contract(Query(contract_query): Query<ContractQuery>) -> impl IntoResponse {
    let chain_id = Chain::try_from(contract_query.chain_id).unwrap();
    let address = Address::from_str(&contract_query.address).unwrap();

    // TODO Check if the contract is verified.

    // Otherwise, decompile and return what we can.
    let provider = provider_from_chain(chain_id);
    let runtime_code = contract_runtime_code(&provider, address).await;
    println!("runtime_code: {:?}", runtime_code);

    if runtime_code == Bytes::from_str("0x").unwrap() {
        return ApiResponse::Error(ErrorResponse {
            error: format!("No runtime code for contract address {address} on chain ID {chain_id}"),
        })
    }

    let temp_dir = TempDir::new().unwrap();
    DecompileBuilder::new(&runtime_code.to_string())
        .output(temp_dir.path().to_str().unwrap()) // comment out this line to have files saved locally
        .include_sol(true)
        .verbosity(0)
        .skip_resolving(false)
        .rpc(&provider_url_from_chain(chain_id))
        .decompile();

    // Read in the files generated by heimdall, their names are always the same:
    //   - abi.json
    //   - bytecode.evm -- this one we don't care about, we already have the bytecode
    //   - decompiled.sol
    //   - disassembled.asm
    let abi = std::fs::read_to_string(temp_dir.path().join("abi.json")).unwrap();
    let solidity = std::fs::read_to_string(temp_dir.path().join("decompiled.sol")).unwrap();
    let disassembled = std::fs::read_to_string(temp_dir.path().join("disassembled.asm")).unwrap();

    // Respond.
    let response =
        DecompiledResponse { chain_id, address, verified: true, abi, disassembled, solidity };
    ApiResponse::Success(response)
}
