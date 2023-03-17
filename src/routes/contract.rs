use crate::provider::{contract_creation_data, provider_from_chain};
use axum::{
    extract::Query,
    http,
    response::{IntoResponse, Response},
    Json,
};
use ethers::types::{Chain, TxHash};
use git2::{Oid, Repository};
use serde::{Deserialize, Serialize};
use std::{error::Error, path::Path, str::FromStr};
use tempfile::TempDir;

#[derive(Deserialize, Debug)]
pub struct ContractQuery {
    chain_id: u64,
    address: String,
}

#[derive(Serialize)]
pub struct ContractResponse {
    chain_id: u64,
    address: String,
    verified: bool,
}

// #[tracing::instrument(name = "Fetching contract")]
pub async fn contract(Query(contract_query): Query<ContractQuery>) -> impl IntoResponse {
    let chain_id = contract_query.chain_id;
    let address = contract_query.address;

    let response = ContractResponse { chain_id, address, verified: false };
    (http::StatusCode::OK, Json(response))
}
