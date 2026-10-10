//! User profile service (Phase 2, docs/PHASE2_PLAN.md Task 2). Handlers stay
//! SQL-free (ARCHITECTURE.md §5): they validate, then call one function here.

use chrono::Utc;
use sqlx::MySqlPool;

use crate::error::ApiError;

/// Set the user's display name. `display_name` arrives already trimmed and
/// validated (1–80 chars, chars not bytes) from the handler.
pub async fn update_display_name(
    pool: &MySqlPool,
    user_id: u64,
    display_name: &str,
) -> Result<(), ApiError> {
    let result = sqlx::query("UPDATE users SET display_name = ?, updated_at = ? WHERE id = ?")
        .bind(display_name)
        .bind(Utc::now())
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|_| ApiError::Internal)?;
    if result.rows_affected() == 0 {
        // Unknown user_id — cannot happen through the CurrentUser extractor
        // (its row was just read), so this is an internal error.
        return Err(ApiError::Internal);
    }
    Ok(())
}
