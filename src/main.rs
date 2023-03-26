use cove::{config, startup, telemetry};
use std::net::TcpListener;

#[tokio::main]
async fn main() -> hyper::Result<()> {
    // TODO Logs are just to stdout right now, we should save them off.
    let subscriber = telemetry::get_subscriber("cove".into(), "info".into(), std::io::stdout);
    telemetry::init_subscriber(subscriber);

    let configuration = config::get_configuration().expect("Failed to read configuration.");
    let address = format!("{}:{}", configuration.application.host, configuration.application.port);
    println!("Listening on {}", address);
    let listener = TcpListener::bind(address).expect("Unable to bind to port");
    startup::run(listener)?.await
}
