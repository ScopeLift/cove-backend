/// This route is intended to return data for a contract that was previously verified, and for
/// unverified contracts falls back to decompiling the bytecode with heimdall. However, Cove does
/// not currently persist verification results in a database. As a result, this route will always
/// decompile the bytecode with heimdall.
pub mod contract;

/// Health check route that returns a 200 OK status code if the server is running.
pub mod health_check;

/// Route for verifying a contract.
pub mod verify;

pub use contract::*;
pub use health_check::*;
pub use verify::*;
