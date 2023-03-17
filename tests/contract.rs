use serde_json::json;
mod common;

// NOTE: Current hashes and contract addresses were randomly pulled from Goerli Etherscan.

#[tokio::test]
async fn contract_returns_a_200_when_all_fields_are_valid() {
    let app = common::spawn_app().await;

    let chain_id = 5;
    let address = "0xc9E7278C9f386f307524eBbAaafcfEb649Be39b4";

    let url = format!("{}/contract?chain_id={}&address={}", app.address, chain_id, address);
    let response = reqwest::get(url).await.unwrap();

    // Check the status code first
    assert_eq!(response.status().as_u16(), 200);

    // Now, extract the response text
    let response_text = response.text().await.unwrap();
    println!("response: {:?}", response_text);

    // assert_eq!(200, response.status().as_u16());

    // let saved = ...
    // assert_eq!();
}
