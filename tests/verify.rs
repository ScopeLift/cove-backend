use std::{collections::HashMap, str::FromStr};

use serde_json::json;
mod common;
use cove::routes::verify::SuccessfulVerification;
use ethers::types::{Chain, TxHash};
use serde_json::from_str;

#[tokio::test]
async fn verify_test() -> Result<(), Box<dyn std::error::Error>> {
    let app = common::spawn_app().await;
    let client = reqwest::Client::new();

    let repo_url = "https://github.com/ScopeLift/cove-test-repo";
    let repo_commit = "94831dee86cba9cb8da031d3bc742a8627156921";
    let contract_address = "0x406B940c7154eDB4Aa2B20CA62fC9A7e70fbe435";
    let mut creation_tx_hashes: HashMap<Chain, TxHash> = HashMap::new();
    creation_tx_hashes.insert(
        Chain::Optimism,
        TxHash::from_str("0xc89b7078fe588ac08d289d220d4f73727d656c698ab027c5a61aabb1dc79c99b")?,
    );
    creation_tx_hashes.insert(
        Chain::Polygon,
        TxHash::from_str("0xf89dda92d455b0634f0bd44fecaca92fd8323e3abeb553bb344584ba5326f127")?,
    );

    let body = json!({
        "repo_url": repo_url,
        "repo_commit": repo_commit,
        "contract_address": contract_address,
        "creation_tx_hashes": Some(creation_tx_hashes),
    });
    // let body = json!({
    //     "repo_url": "https://github.com/ProjectOpenSea/seaport",
    //     "repo_commit": "d58a91d218b0ab557543c8a292710aa36e693973",
    //     "contract_address": "0x00000000000001ad428e4906aE43D8F9852d0dD6",
    // });
    let response = client
        .post(&format!("{}/verify", app.address))
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await?;

    assert_eq!(200, response.status().as_u16());
    let response_body = response.text().await?;
    let verification_result: SuccessfulVerification =
        from_str(&response_body).expect("Failed to deserialize SuccessfulVerification");

    assert_eq!(repo_url, verification_result.repo_url);
    assert_eq!(repo_commit, verification_result.repo_commit);
    Ok(())
}
