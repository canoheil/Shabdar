//! Task 4 (docs/PHASE1_PLAN.md): migration 0001 applies on an EMPTY database
//! and is idempotent. Requires TEST_DATABASE_URL=mysql://…/shabdar_test;
//! without it the DB tests print a note and skip.

mod common;

use sqlx::Row;

use shabdar_api::db::run_migrations;

/// Both DB tests share one database; run them one at a time.
static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Drop everything the migrations own (plus sqlx's bookkeeping table, so the
/// run truly starts at zero), in FK dependency order — child tables first:
/// `sessions.user_id` references `users.id` (fk_sessions_user), so dropping
/// `users` before `sessions` fails with error 1451. Callers run through
/// common::test_pool(), which already refused every database not named *_test.
async fn drop_owned_tables(pool: &sqlx::MySqlPool) {
    for table in ["sessions", "otp_challenges", "users", "_sqlx_migrations"] {
        let sql = format!("DROP TABLE IF EXISTS {table}"); // fixed literals, not user input
        sqlx::query(&sql).execute(pool).await.expect("drop failed");
    }
}

#[tokio::test]
async fn migrations_apply_on_empty_database() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return; // skipped (notice already printed)
    };

    drop_owned_tables(&pool).await;

    run_migrations(&pool).await.expect("first apply must succeed");

    for table in ["users", "otp_challenges", "sessions"] {
        let sql = format!("SELECT COUNT(*) AS c FROM {table}"); // fixed literals
        let row = sqlx::query(&sql)
            .fetch_one(&pool)
            .await
            .expect("table must exist after migration");
        let _ = row.try_get::<i64, _>("c").expect("COUNT column");
    }

    // A second run must be a no-op (idempotent, e.g. on every startup).
    run_migrations(&pool)
        .await
        .expect("second apply must be a no-op");
}

#[tokio::test]
async fn users_and_otp_challenges_have_the_approved_public_id_char26() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return; // skipped
    };

    // No drops: run_migrations is a no-op if the other test already applied.
    run_migrations(&pool).await.expect("migrations apply");

    let row = sqlx::query(
        "SELECT COUNT(*) AS c FROM information_schema.COLUMNS \
         WHERE TABLE_SCHEMA = DATABASE() \
           AND TABLE_NAME IN ('users','otp_challenges') \
           AND COLUMN_NAME = 'public_id' \
           AND COLUMN_TYPE = 'char(26)'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let c: i64 = row.try_get("c").unwrap();
    assert_eq!(c, 2, "both tables expose public_id char(26) (approved amendment)");
}

// --- Environment-free guard tests: these must pass with no database at all.

#[test]
fn db_name_is_parsed_from_mysql_url() {
    assert_eq!(
        common::db_name_from_url("mysql://u:p@127.0.0.1:3306/shabdar_test"),
        Some("shabdar_test")
    );
    assert_eq!(
        common::db_name_from_url("mysql://u@localhost/shabdar_dev?charset=utf8mb4"),
        Some("shabdar_dev")
    );
    assert_eq!(common::db_name_from_url("mysql://u@localhost/"), None);
    assert_eq!(common::db_name_from_url("postgres://u@h/db"), None);
}

#[test]
fn destruction_is_allowed_only_on_test_databases() {
    assert!(common::is_test_database("shabdar_test"));
    assert!(!common::is_test_database("shabdar_dev"));
    assert!(!common::is_test_database("shabdar"));
    assert!(!common::is_test_database(""));
}
