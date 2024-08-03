use axum::{extract::State, Json};

use crate::{
    server::{ApiError, Ctx},
    storage::{Item, Storage},
};

pub const PATH_API_EXPORT: &'static str = "/api/v1/export";

pub async fn handler_api_export(State(ctx): State<Ctx>) -> Result<Json<Vec<Item>>, ApiError> {
    let items = ctx.store.export().await?;

    Ok(Json(items))
}
