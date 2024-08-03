use axum::{extract::State, Json};

use crate::{
    server::{ApiError, Ctx},
    storage::{Item, Storage},
};

#[derive(serde::Serialize)]
pub struct ImportResult {}

pub async fn handler_api_import(
    State(ctx): State<Ctx>,
    input: Json<Vec<Item>>,
) -> Result<Json<ImportResult>, ApiError> {
    ctx.store.import(input.0).await?;
    Ok(Json(ImportResult {}))
}
