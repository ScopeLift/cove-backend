use serde_json::json;
mod common;
use cove::routes::verify::SuccessfulVerification;
use serde_json::from_str;

#[tokio::test]
async fn verify_test() -> Result<(), Box<dyn std::error::Error>> {
    let app = common::spawn_app().await;
    let client = reqwest::Client::new();

    let repo_url = "https://github.com/ScopeLift/cove-test-repo";
    let repo_commit = "188587df6652484e64590127f6ae3038c0aa93e3";
    let contract_address = "0x406B940c7154eDB4Aa2B20CA62fC9A7e70fbe435";

    let body = json!({
        "repo_url": repo_url,
        "repo_commit": repo_commit,
        "contract_address": contract_address,
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
    assert_eq!(contract_address, verification_result.contract_address.to_string());
    Ok(())
}
