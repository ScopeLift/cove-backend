//! # Cove Crate
//!
//! `Cove` is a crate designed for contract verification. It is a work in progress and is not yet
//! ready for production use. See the repository [README](https://github.com/ScopeLift/cove-backend#readme)
//! for more information on the current status. For more details, refer to individual module
//! documentation.
use cove::{config, startup, telemetry};
use std::net::TcpListener;

/// Entrypoint for the application.
#[tokio::main]
async fn main() -> hyper::Result<()> {
    let subscriber = telemetry::get_subscriber("cove".into(), "info".into(), std::io::stdout);
    telemetry::init_subscriber(subscriber);

    let configuration = config::get_configuration().expect("Failed to read configuration.");
    let address = format!("{}:{}", configuration.application.host, configuration.application.port);
    println!("Listening on {}", address);
    let listener = TcpListener::bind(address).expect("Unable to bind to port");
    startup::run(listener)?.await
}
