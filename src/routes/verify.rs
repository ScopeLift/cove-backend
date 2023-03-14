use axum::{http, Form};
use serde::Deserialize;
use std::error::Error;

#[derive(Deserialize)]
pub struct VerifyData {
    repo_url: String,
    repo_commit: Option<String>,
    contract_address: String,
}

#[tracing::instrument(
    name = "Verifying contract",
    skip(form),
    fields(
        repo_url = %form.repo_url,
        repo_commit = %form.repo_commit.as_deref().unwrap_or(""),
        contract_address = %form.contract_address,
    )
)]
pub async fn verify(Form(form): Form<VerifyData>) -> http::StatusCode {
    match do_stuff(&form).await {
        Ok(_) => http::StatusCode::OK,
        Err(_) => http::StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[tracing::instrument(name = "todo", skip(_form))]
pub async fn do_stuff(_form: &VerifyData) -> Result<(), Box<dyn Error>> {
    Ok(())
}
