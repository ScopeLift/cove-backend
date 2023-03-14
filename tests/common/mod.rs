use cove::telemetry::{get_subscriber, init_subscriber};
use once_cell::sync::Lazy;
use std::net::TcpListener;

// Ensure that the `tracing` stack is only initialized once.
static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();

    // We only print logs to the console if the `TEST_LOG` environment variable is set.
    // We cannot assign the output of `get_subscriber` to a variable based on the value `TEST_LOG`
    // because the sink is part of the type returned by `get_subscriber`, therefore they are not the
    // same type. We could work around it, but this is the most straight-forward way of moving
    // forward.
    if std::env::var("TEST_LOG").is_ok() {
        // To see prettified test logs, install bunyan with `cargo install bunyan` then run tests
        // with `TEST_LOG=true cargo test | bunyan`
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
        init_subscriber(subscriber);
    }
});

pub struct TestApp {
    pub address: String,
}

// Launch our application in the background.
// We are running tests, so it is not worth it to propagate errors: if we fail to perform the
// required setup we can just panic and crash all the things.
pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");

    // We retrieve the port assigned to us by the OS.
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{port}");

    // Launch the server as a background task.
    // `tokio::`spawn returns a handle to the spawned future, but we have no use for it here, hence
    // the non-binding `let`.
    let server = cove::startup::run(listener).expect("Failed to bind address");
    let _ = tokio::spawn(server);

    TestApp { address }
}
