mod common;

#[tokio::test]
async fn verify_returns_a_200_when_all_fields_are_valid() {
    let app = common::spawn_app().await;
    let client = reqwest::Client::new();

    let body = "repo_url=qq.com&repo_commit=abc&contract_address=0x123";
    let response = client
        .post(&format!("{}/verify", app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(200, response.status().as_u16());

    // let saved = ...
    // assert_eq!();
}

#[tokio::test]
async fn verify_returns_a_200_when_commit_hash_is_excluded() {
    let app = common::spawn_app().await;
    let client = reqwest::Client::new();

    let body = "repo_url=qq.com&contract_address=0x123";
    let response = client
        .post(&format!("{}/verify", app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(200, response.status().as_u16());

    // let saved = ...
    // assert_eq!();
}

#[tokio::test]
async fn verify_returns_a_400_when_data_is_missing() {
    let app = common::spawn_app().await;
    let client = reqwest::Client::new();

    let test_cases = vec![
        ("contract_address=0x123&repo_commit=0xdef", "missing repo_url"),
        ("repo_url=qq.com&repo_commit=0xdef", "missing contract_address"),
        ("", "missing everything"),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = client
            .post(&format!("{}/verify", app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invalid_body)
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
