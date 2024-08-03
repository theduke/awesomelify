use axum::{extract::State, Json};

use crate::{
    server::{ApiError, Ctx},
    storage::{Item, Storage},
};

pub async fn handler_api_export(State(ctx): State<Ctx>) -> Result<Json<Vec<Item>>, ApiError> {
    let items = ctx.store.export().await?;

    Ok(Json(items))
}
