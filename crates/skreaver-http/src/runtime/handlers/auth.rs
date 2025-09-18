//! Authentication HTTP handlers
//!
//! This module provides authentication endpoints for JWT token creation
//! and related authentication operations.

use axum::{http::StatusCode, response::Json};

use crate::runtime::{auth, types::{CreateTokenRequest, CreateTokenResponse, ErrorResponse}};

/// POST /auth/token - Create a new JWT token
#[utoipa::path(
    post,
    path = "/auth/token",
    request_body = CreateTokenRequest,
    responses(
        (status = 200, description = "Token created successfully", body = CreateTokenResponse),
        (status = 500, description = "Token creation failed", body = ErrorResponse)
    )
)]
pub async fn create_token(
    Json(request): Json<CreateTokenRequest>,
) -> Result<Json<CreateTokenResponse>, (StatusCode, Json<ErrorResponse>)> {
    match auth::create_jwt_token(request.user_id, request.permissions) {
        Ok(token) => Ok(Json(CreateTokenResponse {
            token,
            expires_in: 86400, // 24 hours
            token_type: "Bearer".to_string(),
        })),
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "token_creation_failed".to_string(),
                message: "Failed to create JWT token".to_string(),
                details: None,
            }),
        )),
    }
}