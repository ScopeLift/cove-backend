mod common;

// `tokio::test` is the testing equivalent of `tokio::main`. It also spares you from having to
// specify the `#[test]` attribute. You can inspect what code gets generated using
// `cargo expand --test health_check` (<- name of the test file)
#[tokio::test]
async fn health_check_works() {
    let app = common::spawn_app().await;
    let client = reqwest::Client::new();

    // Send the request.
    let response = client
        .get(&format!("{}/health_check", app.address))
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert on the response.
    assert_eq!(200, response.status().as_u16());
    assert_eq!(Some(0), response.content_length());
}
