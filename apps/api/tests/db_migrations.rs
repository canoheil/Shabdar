//! Task 4 (docs/PHASE1_PLAN.md): migration 0001 applies on an EMPTY database
//! and is idempotent. Requires TEST_DATABASE_URL=mysql://…/shabdar_test;
//! without it the DB tests print a note and skip.

mod common;

use sqlx::Row;

use shabdar_api::db::run_migrations;
use shabdar_api::moderation::{normalize_rule, MatchMode};

/// Both DB tests share one database; run them one at a time.
static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Thin alias over the single centralized reset. Chat tables carry an
/// intentional conversation/message cycle, so the helper pins one connection,
/// disables foreign-key checks while dropping, and restores them before it
/// returns the connection to the pool.
async fn drop_owned_tables(pool: &sqlx::MySqlPool) {
    common::drop_owned_tables(pool).await;
}

#[tokio::test]
async fn migrations_apply_on_empty_database() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return; // skipped (notice already printed)
    };

    drop_owned_tables(&pool).await;

    run_migrations(&pool)
        .await
        .expect("first apply must succeed");

    for table in [
        "users",
        "advertising_slots",
        "advertising_requests",
        "otp_challenges",
        "sessions",
        "provinces",
        "cities",
        "categories",
        "listings",
        "listing_images",
        "favorites",
        "listing_ratings",
        "listing_comments",
        "comment_moderation_cases",
        "moderation_rules",
        "moderation_settings",
        "reports",
        "blocks",
        "conversations",
        "messages",
        "conversation_reads",
        "admin_audit_logs",
    ] {
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
    assert_eq!(
        c, 2,
        "both tables expose public_id char(26) (approved amendment)"
    );
}

#[tokio::test]
async fn reference_tables_are_seeded_for_national_coverage() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return; // skipped
    };

    // No drops: run_migrations is a no-op if the other test already applied.
    run_migrations(&pool).await.expect("migrations apply");

    let row = sqlx::query("SELECT COUNT(*) AS c FROM provinces")
        .fetch_one(&pool)
        .await
        .unwrap();
    let provinces: i64 = row.try_get("c").unwrap();
    assert_eq!(provinces, 31, "seed: every Iranian province");

    let row = sqlx::query("SELECT COUNT(*) AS c FROM cities")
        .fetch_one(&pool)
        .await
        .unwrap();
    let cities: i64 = row.try_get("c").unwrap();
    assert_eq!(cities, 1547, "seed: the pinned national city data");

    let row = sqlx::query(
        "SELECT c.id FROM cities c JOIN provinces p ON p.id = c.province_id \
         WHERE p.slug = 'gilan' AND c.slug = 'rasht'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let rasht_id: u32 = row.try_get("id").unwrap();
    assert_eq!(rasht_id, 1, "the established Rasht identity is preserved");

    let row = sqlx::query("SELECT COUNT(*) AS c FROM categories WHERE is_active = TRUE")
        .fetch_one(&pool)
        .await
        .unwrap();
    let categories: i64 = row.try_get("c").unwrap();
    assert_eq!(categories, 7, "seed: the 7 established categories");

    // Replaying the additive seed directly must not duplicate rows or replace
    // identities, even though the normal migrator records it only once.
    let national_sql = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0013_national_locations.sql"
    ))
    .expect("national location migration");
    sqlx::raw_sql(&national_sql)
        .execute(&pool)
        .await
        .expect("national seed replay");
    let row = sqlx::query(
        "SELECT \
         (SELECT COUNT(*) FROM provinces) AS provinces, \
         (SELECT COUNT(*) FROM cities) AS cities, \
         (SELECT id FROM cities WHERE slug = 'rasht') AS rasht_id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.try_get::<i64, _>("provinces").unwrap(), 31);
    assert_eq!(row.try_get::<i64, _>("cities").unwrap(), 1547);
    assert_eq!(row.try_get::<u32, _>("rasht_id").unwrap(), 1);
}

#[tokio::test]
async fn listings_price_check_rejects_fixed_without_price() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return; // skipped
    };

    run_migrations(&pool).await.expect("migrations apply");

    // Parent rows so that only the CHECK can fire (seeds provide category 1
    // and city 1; the seller is inserted here).
    sqlx::query(
        "INSERT INTO users (public_id, phone_e164, created_at, updated_at) \
         VALUES ('000000000000000000000002', '+989000000001', NOW(3), NOW(3)) \
         ON DUPLICATE KEY UPDATE phone_e164 = VALUES(phone_e164)",
    )
    .execute(&pool)
    .await
    .expect("seller parent row");
    // A `fixed` listing with NULL price must violate chk_listings_price.
    let result = sqlx::query(
        "INSERT INTO listings (public_id, seller_id, category_id, city_id, title, \
         slug, description, price_toman, price_type, status, created_at, updated_at) \
         SELECT '000000000000000000000001', id, 1, 1, 't', 't-slug-check', 'd', \
         NULL, 'fixed', 'draft', NOW(3), NOW(3) FROM users \
         WHERE phone_e164 = '+989000000001'",
    )
    .execute(&pool)
    .await;
    let err = result.expect_err("fixed with NULL price must violate the CHECK constraint");
    assert!(
        err.to_string().contains("chk_listings_price"),
        "the failure must be the CHECK, got: {err}"
    );
}

#[tokio::test]
async fn image_review_schema_is_nullable_but_never_partially_attributed() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    drop_owned_tables(&pool).await;
    run_migrations(&pool).await.expect("migrations apply");

    sqlx::query(
        "INSERT INTO users (public_id, phone_e164, created_at, updated_at) VALUES \
         ('IIIIIIIIIIIIIIIIIIIIIIIIII', '+989111111191', NOW(3), NOW(3))",
    )
    .execute(&pool)
    .await
    .expect("image reviewer fixture");
    let reviewer: u64 =
        sqlx::query_scalar("SELECT id FROM users WHERE public_id = 'IIIIIIIIIIIIIIIIIIIIIIIIII'")
            .fetch_one(&pool)
            .await
            .expect("reviewer ID");
    sqlx::query(
        "INSERT INTO listings (public_id, seller_id, category_id, city_id, title, slug, \
         description, price_toman, price_type, status, created_at, updated_at) VALUES \
         ('HHHHHHHHHHHHHHHHHHHHHHHHHH', ?, 1, 1, 'Legacy images', 'legacy-image-schema', \
         'Description', 1000, 'fixed', 'active', NOW(3), NOW(3))",
    )
    .bind(reviewer)
    .execute(&pool)
    .await
    .expect("legacy listing starts with NULL image review");

    let initial: (
        Option<u64>,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<Vec<u8>>,
    ) = sqlx::query_as(
        "SELECT image_reviewed_by, image_reviewed_at, image_reviewed_ids \
             FROM listings WHERE public_id = 'HHHHHHHHHHHHHHHHHHHHHHHHHH'",
    )
    .fetch_one(&pool)
    .await
    .expect("all legacy review fields exist and are NULL");
    assert!(initial.0.is_none() && initial.1.is_none() && initial.2.is_none());

    sqlx::query(
        "UPDATE listings SET image_reviewed_by = ?, image_reviewed_at = NOW(3), \
         image_reviewed_ids = JSON_ARRAY() \
         WHERE public_id = 'HHHHHHHHHHHHHHHHHHHHHHHHHH'",
    )
    .bind(reviewer)
    .execute(&pool)
    .await
    .expect("complete image review attribution is valid");

    let partial = sqlx::query(
        "UPDATE listings SET image_reviewed_ids = NULL \
         WHERE public_id = 'HHHHHHHHHHHHHHHHHHHHHHHHHH'",
    )
    .execute(&pool)
    .await;
    assert!(partial
        .expect_err("partial review attribution must fail")
        .to_string()
        .contains("chk_listings_image_review"));

    let unknown_reviewer = sqlx::query(
        "UPDATE listings SET image_reviewed_by = 999999 \
         WHERE public_id = 'HHHHHHHHHHHHHHHHHHHHHHHHHH'",
    )
    .execute(&pool)
    .await;
    assert!(unknown_reviewer
        .expect_err("reviewer FK must reject an unknown user")
        .to_string()
        .contains("fk_listings_image_reviewer"));

    let index_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT INDEX_NAME) FROM information_schema.STATISTICS \
         WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'listings' \
           AND INDEX_NAME = 'idx_listings_image_review'",
    )
    .fetch_one(&pool)
    .await
    .expect("image review index metadata");
    assert_eq!(index_count, 1);
}

#[tokio::test]
async fn favorites_rows_cascade_with_user_and_listing() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };

    drop_owned_tables(&pool).await;
    run_migrations(&pool).await.expect("migrations apply");

    // Both listings belong to user 2, so hard-deleting user 1 exercises the
    // favorite FK without colliding with listings.seller_id's RESTRICT rule.
    sqlx::query(
        "INSERT INTO users (public_id, phone_e164, created_at, updated_at) VALUES \
         ('AAAAAAAAAAAAAAAAAAAAAAAAAA', '+989111111101', NOW(3), NOW(3)), \
         ('BBBBBBBBBBBBBBBBBBBBBBBBBB', '+989111111102', NOW(3), NOW(3))",
    )
    .execute(&pool)
    .await
    .expect("two users");
    let user1: u64 =
        sqlx::query_scalar("SELECT id FROM users WHERE public_id = 'AAAAAAAAAAAAAAAAAAAAAAAAAA'")
            .fetch_one(&pool)
            .await
            .expect("user 1 id");
    let user2: u64 =
        sqlx::query_scalar("SELECT id FROM users WHERE public_id = 'BBBBBBBBBBBBBBBBBBBBBBBBBB'")
            .fetch_one(&pool)
            .await
            .expect("user 2 id");

    sqlx::query(
        "INSERT INTO listings (public_id, seller_id, category_id, city_id, title, slug, \
         description, price_toman, price_type, status, created_at, updated_at) VALUES \
         ('CCCCCCCCCCCCCCCCCCCCCCCCCC', ?, 1, 1, 'one', 'favorites-cascade-one', 'd', \
         1000, 'fixed', 'active', NOW(3), NOW(3)), \
         ('DDDDDDDDDDDDDDDDDDDDDDDDDD', ?, 1, 1, 'two', 'favorites-cascade-two', 'd', \
         1000, 'fixed', 'active', NOW(3), NOW(3))",
    )
    .bind(user2)
    .bind(user2)
    .execute(&pool)
    .await
    .expect("two listings");

    for (user_id, listing_public_id) in [
        (user1, "CCCCCCCCCCCCCCCCCCCCCCCCCC"),
        (user2, "DDDDDDDDDDDDDDDDDDDDDDDDDD"),
    ] {
        sqlx::query(
            "INSERT INTO favorites (user_id, listing_id, created_at) \
             SELECT ?, id, NOW(3) FROM listings WHERE public_id = ?",
        )
        .bind(user_id)
        .bind(listing_public_id)
        .execute(&pool)
        .await
        .expect("favorite row");
    }

    let duplicate = sqlx::query(
        "INSERT INTO favorites (user_id, listing_id, created_at) \
         SELECT ?, id, NOW(3) FROM listings \
         WHERE public_id = 'CCCCCCCCCCCCCCCCCCCCCCCCCC'",
    )
    .bind(user1)
    .execute(&pool)
    .await;
    assert!(
        duplicate.is_err(),
        "the composite primary key must reject a duplicate favorite"
    );

    // Schema-only check: a future user-hard-delete application flow must
    // reconcile listing counters first; that unsupported flow is not built here.
    let deleted = sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user1)
        .execute(&pool)
        .await
        .expect("hard-delete user 1");
    assert_eq!(deleted.rows_affected(), 1);
    let deleted =
        sqlx::query("DELETE FROM listings WHERE public_id = 'DDDDDDDDDDDDDDDDDDDDDDDDDD'")
            .execute(&pool)
            .await
            .expect("hard-delete listing 2");
    assert_eq!(deleted.rows_affected(), 1);

    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM favorites")
        .fetch_one(&pool)
        .await
        .expect("favorites count");
    assert_eq!(
        remaining, 0,
        "both cascading foreign keys removed their rows"
    );
}

#[tokio::test]
async fn ratings_and_comments_schema_enforces_invariants() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };

    drop_owned_tables(&pool).await;
    run_migrations(&pool).await.expect("migrations apply");

    sqlx::query(
        "INSERT INTO users (public_id, phone_e164, created_at, updated_at) VALUES \
         ('AAAAAAAAAAAAAAAAAAAAAAAAAA', '+989111111101', NOW(3), NOW(3)), \
         ('BBBBBBBBBBBBBBBBBBBBBBBBBB', '+989111111102', NOW(3), NOW(3))",
    )
    .execute(&pool)
    .await
    .expect("two users");
    let author_id: u64 =
        sqlx::query_scalar("SELECT id FROM users WHERE public_id = 'AAAAAAAAAAAAAAAAAAAAAAAAAA'")
            .fetch_one(&pool)
            .await
            .expect("author id");
    let seller_id: u64 =
        sqlx::query_scalar("SELECT id FROM users WHERE public_id = 'BBBBBBBBBBBBBBBBBBBBBBBBBB'")
            .fetch_one(&pool)
            .await
            .expect("seller id");
    sqlx::query(
        "INSERT INTO listings (public_id, seller_id, category_id, city_id, title, slug, \
         description, price_toman, price_type, status, created_at, updated_at) VALUES \
         ('CCCCCCCCCCCCCCCCCCCCCCCCCC', ?, 1, 1, 'one', 'ratings-comments-schema', 'd', \
         1000, 'fixed', 'active', NOW(3), NOW(3))",
    )
    .bind(seller_id)
    .execute(&pool)
    .await
    .expect("listing parent");
    let listing_id: u64 = sqlx::query_scalar(
        "SELECT id FROM listings WHERE public_id = 'CCCCCCCCCCCCCCCCCCCCCCCCCC'",
    )
    .fetch_one(&pool)
    .await
    .expect("listing id");

    sqlx::query(
        "INSERT INTO listing_ratings (listing_id, user_id, stars, created_at, updated_at) \
         VALUES (?, ?, 5, NOW(3), NOW(3))",
    )
    .bind(listing_id)
    .bind(author_id)
    .execute(&pool)
    .await
    .expect("valid rating");
    let duplicate = sqlx::query(
        "INSERT INTO listing_ratings (listing_id, user_id, stars, created_at, updated_at) \
         VALUES (?, ?, 4, NOW(3), NOW(3))",
    )
    .bind(listing_id)
    .bind(author_id)
    .execute(&pool)
    .await;
    assert!(duplicate.is_err(), "one current rating per user/listing");
    let invalid_stars = sqlx::query(
        "INSERT INTO listing_ratings (listing_id, user_id, stars, created_at, updated_at) \
         VALUES (?, ?, 0, NOW(3), NOW(3))",
    )
    .bind(listing_id)
    .bind(seller_id)
    .execute(&pool)
    .await;
    assert!(
        invalid_stars.is_err(),
        "database rejects stars outside 1..=5"
    );

    sqlx::query(
        "INSERT INTO listing_comments (public_id, listing_id, author_id, body, created_at, updated_at) \
         SELECT 'DDDDDDDDDDDDDDDDDDDDDDDDDD', id, ?, 'comment', NOW(3), NOW(3) \
         FROM listings WHERE public_id = 'CCCCCCCCCCCCCCCCCCCCCCCCCC'",
    )
    .bind(author_id)
    .execute(&pool)
    .await
    .expect("comment row");
    let duplicate_comment_id = sqlx::query(
        "INSERT INTO listing_comments (public_id, listing_id, author_id, body, created_at, updated_at) \
         SELECT 'DDDDDDDDDDDDDDDDDDDDDDDDDD', id, ?, 'other', NOW(3), NOW(3) \
         FROM listings WHERE public_id = 'CCCCCCCCCCCCCCCCCCCCCCCCCC'",
    )
    .bind(author_id)
    .execute(&pool)
    .await;
    assert!(duplicate_comment_id.is_err(), "comment public_id is unique");
    let delete_author = sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(author_id)
        .execute(&pool)
        .await;
    assert!(
        delete_author.is_err(),
        "comment author hard-delete is restricted"
    );

    sqlx::query("DELETE FROM listings WHERE id = ?")
        .bind(listing_id)
        .execute(&pool)
        .await
        .expect("listing hard-delete cascades engagement rows");
    let ratings: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM listing_ratings")
        .fetch_one(&pool)
        .await
        .expect("rating count");
    let comments: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM listing_comments")
        .fetch_one(&pool)
        .await
        .expect("comment count");
    assert_eq!(ratings, 0);
    assert_eq!(comments, 0);
}

#[tokio::test]
async fn reports_and_blocks_schema_enforces_invariants() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };

    drop_owned_tables(&pool).await;
    run_migrations(&pool).await.expect("migrations apply");

    // Direct reads intentionally fail with 1146 before migration 0009 exists.
    let _: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM reports")
        .fetch_one(&pool)
        .await
        .expect("reports table must exist");
    let _: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM blocks")
        .fetch_one(&pool)
        .await
        .expect("blocks table must exist");

    let columns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.COLUMNS \
         WHERE TABLE_SCHEMA = DATABASE() \
           AND ((TABLE_NAME = 'reports' AND COLUMN_NAME = 'public_id' AND COLUMN_TYPE = 'char(26)') \
           OR (TABLE_NAME = 'reports' AND COLUMN_NAME = 'resolution_action' AND COLUMN_TYPE = 'varchar(40)') \
           OR (TABLE_NAME = 'reports' AND COLUMN_NAME = 'details' AND COLUMN_TYPE = 'varchar(500)'))",
    )
    .fetch_one(&pool)
    .await
    .expect("report column metadata");
    assert_eq!(columns, 3, "public ID, resolution, and detail contracts");

    for (table, index) in [
        ("reports", "idx_reports_status_created"),
        ("reports", "idx_reports_target"),
        ("blocks", "idx_blocks_blocked"),
        ("blocks", "idx_blocks_blocker_created"),
    ] {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT INDEX_NAME) FROM information_schema.STATISTICS \
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? AND INDEX_NAME = ?",
        )
        .bind(table)
        .bind(index)
        .fetch_one(&pool)
        .await
        .expect("index metadata");
        assert_eq!(count, 1, "{table} must have {index}");
    }

    sqlx::query(
        "INSERT INTO users (public_id, phone_e164, created_at, updated_at) VALUES \
         ('AAAAAAAAAAAAAAAAAAAAAAAAAA', '+989111111101', NOW(3), NOW(3)), \
         ('BBBBBBBBBBBBBBBBBBBBBBBBBB', '+989111111102', NOW(3), NOW(3))",
    )
    .execute(&pool)
    .await
    .expect("report/block users");
    let reporter: u64 =
        sqlx::query_scalar("SELECT id FROM users WHERE public_id = 'AAAAAAAAAAAAAAAAAAAAAAAAAA'")
            .fetch_one(&pool)
            .await
            .expect("reporter ID");
    let handler: u64 =
        sqlx::query_scalar("SELECT id FROM users WHERE public_id = 'BBBBBBBBBBBBBBBBBBBBBBBBBB'")
            .fetch_one(&pool)
            .await
            .expect("handler ID");

    sqlx::query(
        "INSERT INTO reports \
         (public_id, reporter_id, target_type, target_id, reason, handled_by, created_at) \
         VALUES ('RRRRRRRRRRRRRRRRRRRRRRRRRR', ?, 'listing', 123, 'spam', ?, NOW(3))",
    )
    .bind(reporter)
    .bind(handler)
    .execute(&pool)
    .await
    .expect("one report");
    let state = sqlx::query(
        "SELECT status, resolution_action, handled_by FROM reports \
         WHERE public_id = 'RRRRRRRRRRRRRRRRRRRRRRRRRR'",
    )
    .fetch_one(&pool)
    .await
    .expect("report defaults");
    assert_eq!(state.try_get::<String, _>("status").unwrap(), "open");
    assert_eq!(
        state
            .try_get::<Option<String>, _>("resolution_action")
            .unwrap(),
        None
    );

    let duplicate_report = sqlx::query(
        "INSERT INTO reports (public_id, reporter_id, target_type, target_id, reason, created_at) \
         VALUES ('SSSSSSSSSSSSSSSSSSSSSSSSSS', ?, 'listing', 123, 'other', NOW(3))",
    )
    .bind(reporter)
    .execute(&pool)
    .await;
    assert!(duplicate_report
        .expect_err("one report per reporter/target")
        .to_string()
        .contains("uq_reports_reporter_target"));

    let self_block = sqlx::query(
        "INSERT INTO blocks (blocker_id, blocked_id, created_at) VALUES (?, ?, NOW(3))",
    )
    .bind(reporter)
    .bind(reporter)
    .execute(&pool)
    .await;
    assert!(self_block
        .expect_err("self block must fail")
        .to_string()
        .contains("chk_blocks_no_self"));
    sqlx::query("INSERT INTO blocks (blocker_id, blocked_id, created_at) VALUES (?, ?, NOW(3))")
        .bind(reporter)
        .bind(handler)
        .execute(&pool)
        .await
        .expect("one block");
    let duplicate_block = sqlx::query(
        "INSERT INTO blocks (blocker_id, blocked_id, created_at) VALUES (?, ?, NOW(3))",
    )
    .bind(reporter)
    .bind(handler)
    .execute(&pool)
    .await;
    assert!(duplicate_block.is_err(), "block primary key is unique");

    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(handler)
        .execute(&pool)
        .await
        .expect("handler hard-delete cascades block and nulls handler");
    let handled_by: Option<u64> = sqlx::query_scalar(
        "SELECT handled_by FROM reports WHERE public_id = 'RRRRRRRRRRRRRRRRRRRRRRRRRR'",
    )
    .fetch_one(&pool)
    .await
    .expect("nullable handler");
    assert_eq!(handled_by, None, "handled_by uses ON DELETE SET NULL");
    let blocks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM blocks")
        .fetch_one(&pool)
        .await
        .expect("block count");
    assert_eq!(blocks, 0, "block rows cascade with either user");

    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(reporter)
        .execute(&pool)
        .await
        .expect("reporter hard-delete");
    let reports: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM reports")
        .fetch_one(&pool)
        .await
        .expect("report count");
    assert_eq!(reports, 0, "reports cascade with their reporter");
}

#[tokio::test]
async fn comment_moderation_schema_seed_and_foreign_keys_enforce_invariants() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };

    drop_owned_tables(&pool).await;
    run_migrations(&pool).await.expect("migrations apply");

    // Direct reads intentionally fail with 1146 before migration 0010 exists.
    let seeded_rules: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM moderation_rules")
        .fetch_one(&pool)
        .await
        .expect("moderation rules table and deterministic seed");
    assert_eq!(seeded_rules, 143, "one row per distinct dedupe key");
    let phrase_rules: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM moderation_rules WHERE match_mode = 'phrase'")
            .fetch_one(&pool)
            .await
            .expect("phrase seed count");
    let compact_rules: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM moderation_rules WHERE match_mode = 'compact'")
            .fetch_one(&pool)
            .await
            .expect("compact seed count");
    assert_eq!((phrase_rules, compact_rules), (28, 115));
    let settings_revision: u64 =
        sqlx::query_scalar("SELECT revision FROM moderation_settings WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("singleton settings seed");
    assert_eq!(settings_revision, 1);

    let required_columns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.COLUMNS \
         WHERE TABLE_SCHEMA = DATABASE() AND (\
           (TABLE_NAME = 'moderation_rules' AND COLUMN_NAME IN \
             ('public_id', 'phrase', 'normalized_phrase', 'dedupe_key', 'match_mode', 'is_active', 'revision')) OR \
           (TABLE_NAME = 'comment_moderation_cases' AND COLUMN_NAME IN \
             ('status', 'matched_rules', 'cleared_rules')) OR \
           (TABLE_NAME = 'moderation_settings' AND COLUMN_NAME = 'revision'))",
    )
    .fetch_one(&pool)
    .await
    .expect("moderation column metadata");
    assert_eq!(required_columns, 11, "complete moderation column contract");

    for (table, column, expected) in [
        ("moderation_rules", "public_id", "char(26)"),
        ("moderation_rules", "match_mode", "enum('phrase','compact')"),
        (
            "comment_moderation_cases",
            "status",
            "enum('pending','confirmed_hidden','restored')",
        ),
    ] {
        let actual: String = sqlx::query_scalar(
            "SELECT COLUMN_TYPE FROM information_schema.COLUMNS \
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? AND COLUMN_NAME = ?",
        )
        .bind(table)
        .bind(column)
        .fetch_one(&pool)
        .await
        .expect("moderation column type");
        assert_eq!(actual, expected, "{table}.{column}");
    }

    for (table, index) in [
        ("moderation_rules", "uq_moderation_rules_dedupe"),
        ("moderation_rules", "idx_moderation_rules_active_created"),
        ("comment_moderation_cases", "uq_moderation_cases_comment"),
        (
            "comment_moderation_cases",
            "idx_moderation_cases_status_flagged",
        ),
    ] {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT INDEX_NAME) FROM information_schema.STATISTICS \
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? AND INDEX_NAME = ?",
        )
        .bind(table)
        .bind(index)
        .fetch_one(&pool)
        .await
        .expect("moderation index metadata");
        assert_eq!(count, 1, "{table} must have {index}");
    }

    let duplicate_dedupe = sqlx::query(
        "INSERT INTO moderation_rules \
         (public_id, phrase, normalized_phrase, dedupe_key, match_mode, created_at, updated_at) \
         SELECT 'ZZZZZZZZZZZZZZZZZZZZZZZZZZ', phrase, normalized_phrase, dedupe_key, match_mode, \
                NOW(3), NOW(3) FROM moderation_rules ORDER BY id LIMIT 1",
    )
    .execute(&pool)
    .await;
    assert!(duplicate_dedupe
        .expect_err("dedupe key is unique across modes")
        .to_string()
        .contains("uq_moderation_rules_dedupe"));
    let second_settings =
        sqlx::query("INSERT INTO moderation_settings (id, revision) VALUES (2, 1)")
            .execute(&pool)
            .await;
    assert!(second_settings
        .expect_err("settings must remain a singleton")
        .to_string()
        .contains("chk_moderation_settings_singleton"));

    sqlx::query(
        "INSERT INTO users (public_id, phone_e164, created_at, updated_at) VALUES \
         ('MMMMMMMMMMMMMMMMMMMMMMMMMM', '+989111111201', NOW(3), NOW(3)), \
         ('NNNNNNNNNNNNNNNNNNNNNNNNNN', '+989111111202', NOW(3), NOW(3))",
    )
    .execute(&pool)
    .await
    .expect("moderation author and handler");
    let author: u64 =
        sqlx::query_scalar("SELECT id FROM users WHERE public_id = 'MMMMMMMMMMMMMMMMMMMMMMMMMM'")
            .fetch_one(&pool)
            .await
            .expect("moderation author ID");
    let handler: u64 =
        sqlx::query_scalar("SELECT id FROM users WHERE public_id = 'NNNNNNNNNNNNNNNNNNNNNNNNNN'")
            .fetch_one(&pool)
            .await
            .expect("moderation handler ID");
    sqlx::query(
        "INSERT INTO listings (public_id, seller_id, category_id, city_id, title, slug, \
         description, price_toman, price_type, status, created_at, updated_at) VALUES \
         ('JJJJJJJJJJJJJJJJJJJJJJJJJJ', ?, 1, 1, 'Moderation fixture', \
          'moderation-schema-fixture', 'Description', 1000, 'fixed', 'active', NOW(3), NOW(3))",
    )
    .bind(author)
    .execute(&pool)
    .await
    .expect("moderation listing");
    let listing: u64 = sqlx::query_scalar(
        "SELECT id FROM listings WHERE public_id = 'JJJJJJJJJJJJJJJJJJJJJJJJJJ'",
    )
    .fetch_one(&pool)
    .await
    .expect("moderation listing ID");
    sqlx::query(
        "INSERT INTO listing_comments \
         (public_id, listing_id, author_id, body, created_at, updated_at) \
         VALUES ('PPPPPPPPPPPPPPPPPPPPPPPPPP', ?, ?, 'Pending fixture', NOW(3), NOW(3))",
    )
    .bind(listing)
    .bind(author)
    .execute(&pool)
    .await
    .expect("moderation comment");
    let comment: u64 = sqlx::query_scalar(
        "SELECT id FROM listing_comments WHERE public_id = 'PPPPPPPPPPPPPPPPPPPPPPPPPP'",
    )
    .fetch_one(&pool)
    .await
    .expect("moderation comment ID");
    sqlx::query(
        "INSERT INTO comment_moderation_cases \
         (comment_id, status, matched_rules, handled_by, flagged_at, updated_at) \
         VALUES (?, 'pending', '[]', ?, NOW(3), NOW(3))",
    )
    .bind(comment)
    .bind(handler)
    .execute(&pool)
    .await
    .expect("one moderation case");

    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(handler)
        .execute(&pool)
        .await
        .expect("handler deletion uses SET NULL");
    let handled_by: Option<u64> =
        sqlx::query_scalar("SELECT handled_by FROM comment_moderation_cases WHERE comment_id = ?")
            .bind(comment)
            .fetch_one(&pool)
            .await
            .expect("nullable moderation handler");
    assert_eq!(handled_by, None);

    sqlx::query("DELETE FROM listings WHERE id = ?")
        .bind(listing)
        .execute(&pool)
        .await
        .expect("listing deletion cascades through comments to cases");
    let remaining_comments: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM listing_comments WHERE public_id = 'PPPPPPPPPPPPPPPPPPPPPPPPPP'",
    )
    .fetch_one(&pool)
    .await
    .expect("cascaded comment count");
    let remaining_cases: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM comment_moderation_cases WHERE comment_id = ?")
            .bind(comment)
            .fetch_one(&pool)
            .await
            .expect("cascaded case count");
    assert_eq!((remaining_comments, remaining_cases), (0, 0));
}

#[tokio::test]
async fn every_seed_key_recomputes_with_the_rust_normalizers() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };

    drop_owned_tables(&pool).await;
    run_migrations(&pool).await.expect("migrations apply");
    let rows = sqlx::query(
        "SELECT phrase, normalized_phrase, dedupe_key, match_mode \
         FROM moderation_rules ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .expect("all seeded rules");
    assert_eq!(rows.len(), 143);

    let mut phrase_count = 0;
    let mut compact_count = 0;
    for row in rows {
        let phrase: String = row.try_get("phrase").expect("seed phrase");
        let stored_matcher: String = row.try_get("normalized_phrase").expect("matcher key");
        let stored_dedupe: String = row.try_get("dedupe_key").expect("dedupe key");
        let stored_mode: String = row.try_get("match_mode").expect("match mode");
        let mode = MatchMode::parse(&stored_mode).expect("valid seeded match mode");
        let normalized = normalize_rule(&phrase, mode).expect("valid seeded rule");
        assert_eq!(&normalized.phrase, &phrase);
        assert_eq!(&normalized.normalized_phrase, &stored_matcher, "{phrase}");
        assert_eq!(&normalized.dedupe_key, &stored_dedupe, "{phrase}");
        match mode {
            MatchMode::Phrase => phrase_count += 1,
            MatchMode::Compact => compact_count += 1,
        }
    }
    assert_eq!((phrase_count, compact_count), (28, 115));
}

#[test]
fn moderation_seed_has_complete_upstream_notice() {
    let notice = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../THIRD_PARTY_NOTICES.md"
    ))
    .expect("root third-party notice must exist");
    for required in [
        "https://github.com/kaveh-dev/persian-bad-words",
        "4172f148dd9de014494b12693382cdad2ba1f2f5",
        "Copyright (c) 2022 Thekawasaki",
        "Permission is hereby granted, free of charge",
        "THE SOFTWARE IS PROVIDED \"AS IS\"",
    ] {
        assert!(notice.contains(required), "notice is missing: {required}");
    }
}

// --- Direct chat schema (migration 0014) --------------------------------

/// Inserts one user row and returns its internal id. Public IDs are fixed
/// literals so a failure names the exact fixture.
async fn chat_user(pool: &sqlx::MySqlPool, public_id: &str, phone: &str) -> u64 {
    let result = sqlx::query(
        "INSERT INTO users (public_id, phone_e164, display_name, created_at, updated_at) \
         VALUES (?, ?, 'کاربر آزمون', NOW(3), NOW(3))",
    )
    .bind(public_id)
    .bind(phone)
    .execute(pool)
    .await
    .expect("chat user fixture");
    result.last_insert_id()
}

async fn chat_listing(pool: &sqlx::MySqlPool, seller_id: u64, slug: &str) -> u64 {
    let public_id = ulid::Ulid::new().to_string();
    let result = sqlx::query(
        "INSERT INTO listings (public_id, seller_id, category_id, city_id, title, slug, \
         description, price_toman, price_type, status, published_at, created_at, updated_at) \
         VALUES (?, ?, 1, 1, 'chat fixture', ?, 'description', 1000, 'fixed', 'active', \
         NOW(3), NOW(3), NOW(3))",
    )
    .bind(public_id)
    .bind(seller_id)
    .bind(slug)
    .execute(pool)
    .await
    .expect("chat listing fixture");
    result.last_insert_id()
}

#[tokio::test]
async fn chat_tables_apply_on_empty_database() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return; // skipped
    };

    common::drop_owned_tables(&pool).await;
    run_migrations(&pool).await.expect("first apply");

    let mut tables: Vec<String> = sqlx::query_scalar(
        "SELECT TABLE_NAME FROM information_schema.TABLES \
         WHERE TABLE_SCHEMA = DATABASE() \
           AND TABLE_NAME IN ('conversations','messages','conversation_reads')",
    )
    .fetch_all(&pool)
    .await
    .expect("chat table metadata");
    // `information_schema` ordering follows the server collation, which treats
    // the underscore differently across versions; sort in Rust and compare.
    tables.sort();
    assert_eq!(
        tables,
        vec![
            "conversation_reads".to_string(),
            "conversations".to_string(),
            "messages".to_string()
        ],
        "migration 0014 must create exactly the three chat tables"
    );

    let public_ids: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.COLUMNS \
         WHERE TABLE_SCHEMA = DATABASE() \
           AND TABLE_NAME IN ('conversations','messages') \
           AND COLUMN_NAME = 'public_id' AND COLUMN_TYPE = 'char(26)'",
    )
    .fetch_one(&pool)
    .await
    .expect("public_id metadata");
    assert_eq!(public_ids, 2, "both chat tables expose public_id char(26)");

    for index in [
        "public_id",
        "uq_conversations_pair",
        "idx_conversations_low_recency",
        "idx_conversations_high_recency",
        "idx_conversations_low_sync",
        "idx_conversations_high_sync",
    ] {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT INDEX_NAME) FROM information_schema.STATISTICS \
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'conversations' \
               AND INDEX_NAME = ?",
        )
        .bind(index)
        .fetch_one(&pool)
        .await
        .expect("conversation index metadata");
        assert_eq!(count, 1, "missing conversations index {index}");
    }
    for index in [
        "public_id",
        "uq_messages_client_id",
        "idx_messages_conversation_order",
        "idx_messages_listing",
    ] {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT INDEX_NAME) FROM information_schema.STATISTICS \
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'messages' \
               AND INDEX_NAME = ?",
        )
        .bind(index)
        .fetch_one(&pool)
        .await
        .expect("message index metadata");
        assert_eq!(count, 1, "missing messages index {index}");
    }

    // The unique pair and client-ID boundaries are real database constraints.
    let unique_indexes: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT INDEX_NAME) FROM information_schema.STATISTICS \
         WHERE TABLE_SCHEMA = DATABASE() \
           AND ((TABLE_NAME = 'conversations' AND INDEX_NAME = 'uq_conversations_pair') \
             OR (TABLE_NAME = 'messages' AND INDEX_NAME = 'uq_messages_client_id')) \
           AND NON_UNIQUE = 0",
    )
    .fetch_one(&pool)
    .await
    .expect("unique index metadata");
    assert_eq!(unique_indexes, 2);

    // Second application is a no-op and keeps the national seed intact.
    run_migrations(&pool)
        .await
        .expect("second apply must be a no-op");
    let counts: (i64, i64) =
        sqlx::query_as("SELECT (SELECT COUNT(*) FROM provinces), (SELECT COUNT(*) FROM cities)")
            .fetch_one(&pool)
            .await
            .expect("national counts");
    assert_eq!(counts, (31, 1547));
}

#[tokio::test]
async fn chat_constraints_and_repeat_reset_are_safe() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return; // skipped
    };

    for round in 0..2 {
        common::reset_database(&pool).await;
        let restored: bool = sqlx::query_scalar("SELECT @@FOREIGN_KEY_CHECKS = 1")
            .fetch_one(&pool)
            .await
            .expect("foreign key check state");
        assert!(restored, "FK checks must be restored after reset {round}");
        let rasht: u32 = sqlx::query_scalar("SELECT id FROM cities WHERE slug = 'rasht'")
            .fetch_one(&pool)
            .await
            .expect("Rasht identity after reset");
        assert_eq!(rasht, 1);
    }

    let seller = chat_user(&pool, "CHATSCHEMA0000000000000001", "+989120000001").await;
    let buyer = chat_user(&pool, "CHATSCHEMA0000000000000002", "+989120000002").await;
    let listing = chat_listing(&pool, seller, "chat-schema-listing").await;
    let (low, high) = if seller < buyer {
        (seller, buyer)
    } else {
        (buyer, seller)
    };

    // Ordered pair: equal or reversed ids are rejected.
    let unordered = sqlx::query(
        "INSERT INTO conversations (public_id, user_low_id, user_high_id, initiator_id, \
         created_at, updated_at) VALUES ('CHATORDER000000000000000001', ?, ?, ?, NOW(3), NOW(3))",
    )
    .bind(high)
    .bind(low)
    .bind(buyer)
    .execute(&pool)
    .await;
    // MariaDB reports a CHECK violation as `CONSTRAINT ... failed` (4025) and
    // MySQL as error 3819; both are constraint failures, never a plain insert.
    let unordered_error = format!(
        "{:?}",
        unordered.expect_err("an unordered pair must violate the CHECK")
    );
    assert!(
        unordered_error.contains("chk_conversations_ordered_pair")
            || unordered_error.contains("4025")
            || unordered_error.contains("3819"),
        "an unordered pair must be rejected by the ordered-pair CHECK: {unordered_error}"
    );

    let conversation_id: u64 = {
        let result = sqlx::query(
            "INSERT INTO conversations (public_id, user_low_id, user_high_id, initiator_id, \
             created_at, updated_at) VALUES ('CHATPAIR000000000000000001', ?, ?, ?, NOW(3), NOW(3))",
        )
        .bind(low)
        .bind(high)
        .bind(buyer)
        .execute(&pool)
        .await
        .expect("canonical conversation");
        result.last_insert_id()
    };

    // Duplicate pair in either initiation order is impossible.
    let duplicate = sqlx::query(
        "INSERT INTO conversations (public_id, user_low_id, user_high_id, initiator_id, \
         created_at, updated_at) VALUES ('CHATPAIR000000000000000002', ?, ?, ?, NOW(3), NOW(3))",
    )
    .bind(low)
    .bind(high)
    .bind(seller)
    .execute(&pool)
    .await;
    assert!(duplicate
        .expect_err("duplicate canonical pair must fail")
        .to_string()
        .contains("uq_conversations_pair"));

    // Both participant read rows start with NULL high-water state.
    for user in [low, high] {
        sqlx::query("INSERT INTO conversation_reads (conversation_id, user_id) VALUES (?, ?)")
            .bind(conversation_id)
            .bind(user)
            .execute(&pool)
            .await
            .expect("initial read row");
    }

    // A partial last-message pair is rejected.
    let partial_last = sqlx::query(
        "UPDATE conversations SET last_message_at = NOW(3) \
         WHERE public_id = 'CHATPAIR000000000000000001'",
    )
    .execute(&pool)
    .await;
    assert!(partial_last
        .expect_err("last_message_id without timestamp must fail")
        .to_string()
        .contains("chk_conversations_last_message_pair"));

    let message_id: u64 = {
        let result = sqlx::query(
            "INSERT INTO messages (public_id, conversation_id, sender_id, listing_id, \
             client_message_id, body, created_at) VALUES \
             ('CHATMESSAGE000000000000001', ?, ?, ?, \
             '123e4567-e89b-42d3-a456-426614174000', 'سلام', NOW(3))",
        )
        .bind(conversation_id)
        .bind(buyer)
        .bind(listing)
        .execute(&pool)
        .await
        .expect("first message");
        result.last_insert_id()
    };
    sqlx::query(
        "UPDATE conversations SET last_message_id = ?, last_message_at = NOW(3) \
         WHERE id = ?",
    )
    .bind(message_id)
    .bind(conversation_id)
    .execute(&pool)
    .await
    .expect("last message pointer");

    // The same client ID is unique per conversation + sender and reusable by
    // the other participant (the stated idempotency scope).
    let duplicate_client = sqlx::query(
        "INSERT INTO messages (public_id, conversation_id, sender_id, client_message_id, \
         body, created_at) VALUES ('CHATMESSAGE000000000000002', ?, ?, \
         '123e4567-e89b-42d3-a456-426614174000', 'دوباره', NOW(3))",
    )
    .bind(conversation_id)
    .bind(buyer)
    .execute(&pool)
    .await;
    assert!(duplicate_client
        .expect_err("duplicate client ID must fail for the same sender")
        .to_string()
        .contains("uq_messages_client_id"));
    sqlx::query(
        "INSERT INTO messages (public_id, conversation_id, sender_id, client_message_id, \
         body, created_at) VALUES ('CHATMESSAGE000000000000003', ?, ?, \
         '123e4567-e89b-42d3-a456-426614174000', 'طرف دیگر', NOW(3))",
    )
    .bind(conversation_id)
    .bind(seller)
    .execute(&pool)
    .await
    .expect("the other sender may reuse the same UUID");

    // Read state is paired or NULL, never half-set.
    let partial_read = sqlx::query(
        "UPDATE conversation_reads SET last_read_at = NOW(3) WHERE conversation_id = ?",
    )
    .bind(conversation_id)
    .execute(&pool)
    .await;
    assert!(partial_read
        .expect_err("read id and timestamp must advance together")
        .to_string()
        .contains("chk_conversation_reads_pair"));

    // A further destructive reset clears every chat row even though the two
    // tables reference each other, and restores the national seed afterwards.
    common::reset_database(&pool).await;
    let remaining: i64 = sqlx::query_scalar(
        "SELECT CAST((SELECT COUNT(*) FROM messages) + (SELECT COUNT(*) FROM conversations) \
         + (SELECT COUNT(*) FROM conversation_reads) AS SIGNED)",
    )
    .fetch_one(&pool)
    .await
    .expect("chat counts after reset");
    assert_eq!(remaining, 0, "reset must drop every chat row");
    let restored: bool = sqlx::query_scalar("SELECT @@FOREIGN_KEY_CHECKS = 1")
        .fetch_one(&pool)
        .await
        .expect("foreign key checks");
    assert!(restored);
    let cities: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cities")
        .fetch_one(&pool)
        .await
        .expect("national seed");
    assert_eq!(cities, 1547);
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
