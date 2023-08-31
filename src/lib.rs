#![doc = include_str!("../README.md")]

/// Contains methods and types for analyzing and comparing bytecode.
pub mod bytecode;

/// Handles all app configuration.
pub mod config;

/// Defines the `Framework` trait for abstracting over different development frameworks. Also
/// contains an implementation for Foundry.
pub mod frameworks;

/// Contains methods and types for interacting with an Ethereum provider and comparing bytecode.
pub mod provider;

/// Defines the handlers for all API routes.
pub mod routes;

/// Handles the server startup, such as route configuration and middleware.
pub mod startup;

/// Handles logs and tracing.
pub mod telemetry;
