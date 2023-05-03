use serde_json::json;
mod common;
use cove::routes::verify::SuccessfulVerification;
use serde_json::from_str;

#[tokio::test]
async fn verify_counter() -> Result<(), Box<dyn std::error::Error>> {
    let app = common::spawn_app().await;
    let client = reqwest::Client::new();

    // POST request inputs.
    let repo_url = "https://github.com/ScopeLift/cove-test-repo";
    let repo_commit = "94831dee86cba9cb8da031d3bc742a8627156921";
    let contract_address = "0x406B940c7154eDB4Aa2B20CA62fC9A7e70fbe435";
    let build_config = json!({
        "framework": "foundry",
        "build_hint": "default"
    });
    let creation_tx_hashes = json!({
        "optimism": "0xc89b7078fe588ac08d289d220d4f73727d656c698ab027c5a61aabb1dc79c99b",
        "polygon": "0xf89dda92d455b0634f0bd44fecaca92fd8323e3abeb553bb344584ba5326f127"
    });

    let body = json!({
        "repo_url": repo_url,
        "repo_commit": repo_commit,
        "contract_address": contract_address,
        "build_config": build_config,
        "creation_tx_hashes": Some(creation_tx_hashes),
    });

    // Send request.
    let response = client
        .post(&format!("{}/verify", app.address))
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await?;

    // Assertions.
    assert_eq!(200, response.status().as_u16());
    let response_body = response.text().await?;
    let verification_result: SuccessfulVerification =
        from_str(&response_body).expect("Failed to deserialize SuccessfulVerification");

    assert_eq!(repo_url, verification_result.repo_url);
    assert_eq!(repo_commit, verification_result.repo_commit);
    Ok(())
}

#[tokio::test]
async fn verify_seaport() -> Result<(), Box<dyn std::error::Error>> {
    let app = common::spawn_app().await;
    let client = reqwest::Client::new();

    // POST request inputs.
    let repo_url = "https://github.com/ProjectOpenSea/seaport";
    let repo_commit = "d58a91d218b0ab557543c8a292710aa36e693973";
    let contract_address = "0x00000000000001ad428e4906aE43D8F9852d0dD6";
    let build_config = json!({
        "framework": "foundry",
        "build_hint": "optimized"
    });
    let creation_tx_hashes = json!({
        "mainnet": "0x4f5eae3d221fe4a572d722a57c2fbfd252139e7580b7959d93eb2a8b05b666f6",
        "polygon": "0x7c0a769c469d24859cbcb978caacd9b6d5eea1f50ae6c1b3c94d4819375e0b09",
        "optimism": "0x3a46979922e781895fae9cba54df645b813eb55447703f590d51af1993ad59d4",
        "arbitrum": "0xa150f5c8bf8b8a0fc5f4f64594d09d796476974280e566fe3899b56517cd11da",
        "gnosis_chain": "0xfc189820c60536e2ce90443ac3d39633583cfed6653d5f7edd7c0e115fd2a18b",
    });

    let body = json!({
        "repo_url": repo_url,
        "repo_commit": repo_commit,
        "contract_address": contract_address,
        "build_config": build_config,
        "creation_tx_hashes": Some(creation_tx_hashes),
    });

    // Send request.
    let response = client
        .post(&format!("{}/verify", app.address))
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await?;

    // Assertions.
    assert_eq!(200, response.status().as_u16());
    let response_body = response.text().await?;
    println!("response_body {:?}", response_body);
    let verification_result: SuccessfulVerification =
        from_str(&response_body).expect("Failed to deserialize SuccessfulVerification");

    assert_eq!(repo_url, verification_result.repo_url);
    assert_eq!(repo_commit, verification_result.repo_commit);
    Ok(())
}
