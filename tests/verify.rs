use serde_json::json;
mod common;

// NOTE: Current hashes and contract addresses were randomly pulled from Goerli Etherscan.

#[tokio::test]
async fn verify_returns_a_200_when_all_fields_are_valid() {
    let app = common::spawn_app().await;
    let client = reqwest::Client::new();

    let body = json!({
        "repo_url": "https://github.com/ScopeLift/cove-test-repo",
        "repo_commit": "14a113dd794d4938da7e0e12828434d666eb9a31",
        "contract_address": "0x1908e2bf4a88f91e4ef0dc72f02b8ea36bea2319",
        "chain_id": 5,
        "creation_tx_hash": "0x071327401d96fbc16ebbd0eea06deb1d5fe1e78593d13de5585a23c8459fd390"
    });
    let response = client
        .post(&format!("{}/verify", app.address))
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(200, response.status().as_u16());

    // let saved = ...
    // assert_eq!();
}

#[tokio::test]
async fn verify_returns_a_400_when_repo_cannot_be_cloned() {
    let app = common::spawn_app().await;
    let client = reqwest::Client::new();

    let body = json!({
        "repo_url": "https://github.com/ScopeLift/non-existant-repo",
        "repo_commit": "14a113dd794d4938da7e0e12828434d666eb9a31",
        "contract_address": "0x1908e2bf4a88f91e4ef0dc72f02b8ea36bea2319",
        "chain_id": 5,
        "creation_tx_hash": "0x071327401d96fbc16ebbd0eea06deb1d5fe1e78593d13de5585a23c8459fd390"
    });
    let response = client
        .post(&format!("{}/verify", app.address))
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(400, response.status().as_u16(),);
}

#[tokio::test]
async fn verify_returns_a_400_when_repo_has_no_foundry_toml() {
    let app = common::spawn_app().await;
    let client = reqwest::Client::new();

    let body = json!({
        "repo_url": "https://github.com/ScopeLift/scopelint",
        "repo_commit": "14a113dd794d4938da7e0e12828434d666eb9a31",
        "contract_address": "0x1908e2bf4a88f91e4ef0dc72f02b8ea36bea2319",
        "chain_id": 5,
        "creation_tx_hash": "0x071327401d96fbc16ebbd0eea06deb1d5fe1e78593d13de5585a23c8459fd390"
    });
    let response = client
        .post(&format!("{}/verify", app.address))
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(400, response.status().as_u16(),);
}
#[tokio::test]
async fn verify_returns_a_400_when_data_is_missing() {
    let app = common::spawn_app().await;
    let client = reqwest::Client::new();

    let body1 = json!({
        "repo_commit": "abcdef1",
        "contract_address": "0x123",
        "chain_id": "5",
        "creation_tx_hash": "0x071327401d96fbc16ebbd0eea06deb1d5fe1e78593d13de5585a23c8459fd390"
    });

    let body2 = json!({
        "repo_url": "https://github.com/ScopeLift/cove-test-repo",
        "repo_commit": "abcdef1",
        "contract_address": "0x1908e2bf4a88f91e4ef0dc72f02b8ea36bea2319",
        "chain_id": "5",
    });

    // TODO Test more combinations.
    let test_cases = vec![(body1, "missing repo_url"), (body2, "missing creation_tx_hash")];

    for (invalid_body, error_message) in test_cases {
        let response = client
            .post(&format!("{}/verify", app.address))
            .header("Content-Type", "application/json")
            .body(invalid_body.to_string())
            .send()
            .await
            .expect("Failed to execute request.");

        assert_eq!(
            422,
            response.status().as_u16(),
            "Wrong response for payload:
{error_message}"
        );
    }
}
