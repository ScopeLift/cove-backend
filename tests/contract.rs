mod common;

#[tokio::test]
async fn contract_test() {
    let app = common::spawn_app().await;

    let chain_id = 5;
    let address = "0xc9E7278C9f386f307524eBbAaafcfEb649Be39b4";

    let url = format!("{}/contract?chain_id={}&address={}", app.address, chain_id, address);
    let response = reqwest::get(url).await.unwrap();

    let response_text = response.text().await.unwrap();
    println!("response: {:?}", response_text);
    // assert_eq!(response.status().as_u16(), 200);
}
