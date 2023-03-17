use crate::routes;
use axum::{
    routing::{get, post, IntoMakeService},
    Router, Server,
};
use hyper::server::conn::AddrIncoming;
use std::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
    request_id::MakeRequestUuid,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
    ServiceBuilderExt,
};

pub fn run(listener: TcpListener) -> hyper::Result<Server<AddrIncoming, IntoMakeService<Router>>> {
    // Configure service to have request IDs show up correctly in logs produced by
    // `tower_http::trace::Trace`. Modified from: https://docs.rs/tower-http/latest/tower_http/request_id/index.html#using-trace
    let trace_layer = ServiceBuilder::new()
        .set_x_request_id(MakeRequestUuid)
        // Log requests and responses.
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().include_headers(true))
                .on_response(DefaultOnResponse::new().include_headers(true)),
        )
        // Propagate the header to the response before the response reaches `TraceLayer`.
        .propagate_x_request_id();

    // Build our application with a single route.
    let app = Router::new()
        .route("/health_check", get(routes::health_check))
        .route("/verify", post(routes::verify))
        .route("/contract", get(routes::contract))
        .layer(trace_layer);

    // Run it with hyper on the given TcpListener.
    Ok(axum::Server::from_tcp(listener)?.serve(app.into_make_service()))
}
