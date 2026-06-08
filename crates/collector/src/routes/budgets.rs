//! T-351 — Budget routes (CRUD + evaluation)

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::app::AppState;
use crate::errors::AppError;
use crate::services::budget_service::{self, BudgetStatus, Budget, CreateBudget, UpdateBudget};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/budgets", get(list_budgets).post(create_budget))
        .route("/api/budgets/status", get(budget_status))
        .route(
            "/api/budgets/{id}",
            get(get_budget).put(update_budget).delete(delete_budget),
        )
}

async fn list_budgets(State(state): State<AppState>) -> Result<Json<Vec<Budget>>, AppError> {
    let budgets = budget_service::list(&state.pool).await?;
    Ok(Json(budgets))
}

async fn get_budget(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Budget>, AppError> {
    let budget = budget_service::get(&state.pool, id).await?;
    Ok(Json(budget))
}

async fn create_budget(
    State(state): State<AppState>,
    Json(input): Json<CreateBudget>,
) -> Result<Json<Budget>, AppError> {
    let budget = budget_service::create(&state.pool, input).await?;
    Ok(Json(budget))
}

async fn update_budget(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateBudget>,
) -> Result<Json<Budget>, AppError> {
    let budget = budget_service::update(&state.pool, id, input).await?;
    Ok(Json(budget))
}

async fn delete_budget(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    budget_service::delete(&state.pool, id).await?;
    Ok(Json(serde_json::json!({"deleted": true})))
}

async fn budget_status(
    State(state): State<AppState>,
) -> Result<Json<Vec<BudgetStatus>>, AppError> {
    let statuses = budget_service::evaluate_all(&state.pool).await?;
    Ok(Json(statuses))
}
