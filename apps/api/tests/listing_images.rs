//! Listing image uploads on local disk, full static-image validation,
//! dimensions, media authorization, reorder, and delete (Phase 11).

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

/// All upload tests share one database; run them one at a time.
static UPLOAD_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Same child-first reset as the other suites (see tests/db_migrations.rs).
async fn reset_db(pool: &sqlx::MySqlPool) {
    for table in [
        "comment_moderation_cases",
        "moderation_rules",
        "moderation_settings",
        "reports",
        "blocks",
        "listing_comments",
        "listing_ratings",
        "favorites",
        "listing_images",
        "listings",
        "admin_audit_logs",
        "sessions",
        "otp_challenges",
        "cities",
        "categories",
        "provinces",
        "users",
        "_sqlx_migrations",
    ] {
        let sql = format!("DROP TABLE IF EXISTS {table}"); // fixed literals
        sqlx::query(&sql).execute(pool).await.expect("drop failed");
    }
    shabdar_api::db::run_migrations(pool)
        .await
        .expect("migrations apply");
}

async fn challenge_with_code(
    pool: &sqlx::MySqlPool,
    cfg: &shabdar_api::config::Config,
    phone: &str,
    purpose: &str,
) -> (String, String) {
    let code = shabdar_api::otp::generate_code();
    let created = shabdar_api::otp_service::create_challenge(pool, cfg, phone, purpose, &code)
        .await
        .expect("create challenge");
    (created.public_id, code)
}

/// Register through the real OTP flow; return (cookie header, csrf token).
async fn register_user(
    app: &axum::Router,
    pool: &sqlx::MySqlPool,
    cfg: &shabdar_api::config::Config,
    phone: &str,
) -> (String, String) {
    let (challenge_id, code) = challenge_with_code(pool, cfg, phone, "register").await;
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/otp/verify")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({ "challenge_id": challenge_id, "code": code }).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let mut session = String::new();
    let mut csrf = String::new();
    for value in resp.headers().get_all(axum::http::header::SET_COOKIE) {
        let text = value.to_str().expect("ascii cookie");
        if let Some(rest) = text.strip_prefix("shabdar_session=") {
            session = format!("shabdar_session={}", rest.split(';').next().unwrap());
        } else if let Some(rest) = text.strip_prefix("shabdar_csrf=") {
            csrf = rest.split(';').next().unwrap().to_string();
        }
    }
    (format!("{session}; shabdar_csrf={csrf}"), csrf)
}

/// A draft listing owned by the cookie holder; returns its public_id.
async fn create_draft(app: &axum::Router, cookie: &str, csrf: &str) -> String {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/listings")
        .header("content-type", "application/json")
        .header("cookie", cookie)
        .header("x-csrf-token", csrf)
        .body(Body::from(
            serde_json::json!({
                "category_id": 1, "city_id": 1,
                "title": "فرش دستباف", "description": "در حد نو",
                "price_toman": 5_000_000, "price_type": "fixed",
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    body["public_id"].as_str().unwrap().to_string()
}

/// Small real static images generated once for the checked-in test contract.
/// The API must decode these bytes; filenames and multipart MIME are not proof.
fn png_bytes() -> Vec<u8> {
    include_bytes!("fixtures/tiny.png").to_vec()
}
fn jpeg_bytes() -> Vec<u8> {
    include_bytes!("fixtures/tiny.jpg").to_vec()
}
fn webp_bytes() -> Vec<u8> {
    include_bytes!("fixtures/tiny.webp").to_vec()
}

/// POST one multipart file field; returns status + parsed JSON.
async fn upload(
    app: axum::Router,
    public_id: &str,
    cookie: &str,
    csrf: Option<&str>,
    filename: &str,
    content_type: &str,
    bytes: &[u8],
) -> (StatusCode, serde_json::Value) {
    let boundary = "shabdar-test-boundary";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(format!("Content-Type: {content_type}\r\n\r\n").as_bytes());
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let mut builder = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/listings/{public_id}/images"))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .header("cookie", cookie);
    if let Some(t) = csrf {
        builder = builder.header("x-csrf-token", t);
    }
    let resp = app
        .oneshot(builder.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).expect("JSON body"))
}

/// A portable scratch path; the process id keeps parallel test binaries apart.
fn scratch_media_dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("shabdar-{name}-{}", std::process::id()))
}

/// Config pointing MEDIA_DIR at a fresh scratch directory (deleted first).
fn media_cfg(dir: &std::path::Path) -> shabdar_api::config::Config {
    let _ = std::fs::remove_dir_all(dir);
    let mut cfg = common::test_config();
    cfg.media_dir = dir.to_string_lossy().into_owned();
    cfg
}

/// GET a path; return status + raw bytes + content-type.
async fn get_raw(app: axum::Router, path: &str) -> (StatusCode, Vec<u8>, Option<String>) {
    let req = Request::builder()
        .method(Method::GET)
        .uri(path)
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let bytes = BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, bytes, content_type)
}

#[tokio::test]
async fn uploaded_file_is_served_back_with_bytes() {
    let _lock = UPLOAD_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let dir = scratch_media_dir("media-png");
    let cfg = media_cfg(&dir);
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let public_id = create_draft(&app, &cookie, &csrf).await;

    let (status, body) = upload(
        app.clone(),
        &public_id,
        &cookie,
        Some(&csrf),
        "a.png",
        "image/png",
        &png_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let key = body["storage_key"].as_str().expect("storage_key");
    assert_eq!(body["url"], serde_json::json!(format!("/media/{key}")));
    assert!(
        dir.join(key).is_file(),
        "file must exist under MEDIA_DIR: {key}"
    );
    assert_eq!(
        (body["width"].as_u64(), body["height"].as_u64()),
        (Some(3), Some(2))
    );
    let stored: (u16, u16) =
        sqlx::query_as("SELECT width, height FROM listing_images WHERE storage_key = ?")
            .bind(key)
            .fetch_one(&pool)
            .await
            .expect("decoded dimensions are persisted");
    assert_eq!(stored, (3, 2));
    // WebP rides the same path (third accepted signature).
    let (status, body) = upload(
        app.clone(),
        &public_id,
        &cookie,
        Some(&csrf),
        "a.webp",
        "image/webp",
        &webp_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["url"].as_str().unwrap().ends_with(".webp"));
    assert_eq!(
        (body["width"].as_u64(), body["height"].as_u64()),
        (Some(5), Some(4))
    );

    // The minted URL serves the exact bytes back (no auth needed — reading a
    // listing's photo is public, like the listing itself).
    let url = body["url"].as_str().unwrap().to_string();
    let (status, bytes, content_type) = get_raw(app.clone(), &url).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, webp_bytes());
    assert_eq!(content_type.as_deref(), Some("image/webp"));

    // Unknown keys, wrong extensions, and traversal shapes are all 404 —
    // never a file outside MEDIA_DIR, never a 500.
    for path in [
        "/media/AAAAAAAAAAAAAAAAAAAAAAAAAA/BBBBBBBBBBBBBBBBBBBBBBBBBB.png".to_string(),
        format!("/media/{}.exe", &key[..key.len() - 4]),
        "/media/../../etc/passwd".to_string(),
        "/media/%2e%2e/%2e%2e/etc/passwd".to_string(),
    ] {
        let (status, _, _) = get_raw(app.clone(), &path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn upload_rejects_exe_bytes_and_wrong_owner_and_active_listing() {
    let _lock = UPLOAD_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let dir = scratch_media_dir("media-neg");
    let cfg = media_cfg(&dir);
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let (other_cookie, other_csrf) = register_user(&app, &pool, &cfg, "+989009876543").await;
    let public_id = create_draft(&app, &cookie, &csrf).await;

    // MZ bytes are not an image.
    let (status, body) = upload(
        app.clone(),
        &public_id,
        &cookie,
        Some(&csrf),
        "evil.exe",
        "application/octet-stream",
        b"MZ\x90\x00",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "validation_error");
    assert_eq!(body["error"]["fields"]["images"], "invalid_type");

    // Another seller's listing → 403, no file written.
    let (status, body) = upload(
        app.clone(),
        &public_id,
        &other_cookie,
        Some(&other_csrf),
        "a.png",
        "image/png",
        &png_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "forbidden");

    // Active listings are immutable (v1): flip then upload → 400.
    sqlx::query("UPDATE listings SET status = 'active', published_at = NOW(3) WHERE public_id = ?")
        .bind(&public_id)
        .execute(&pool)
        .await
        .expect("activate");
    let (status, _) = upload(
        app,
        &public_id,
        &cookie,
        Some(&csrf),
        "a.png",
        "image/png",
        &png_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let _ = std::fs::remove_dir_all(dir);
}

async fn assert_image_rejections(
    app: axum::Router,
    pool: &sqlx::MySqlPool,
    dir: &std::path::Path,
    public_id: &str,
    cookie: &str,
    csrf: &str,
    cases: &[(&str, &[u8], &str)],
) {
    let mut failures = Vec::new();
    for (filename, bytes, expected) in cases.iter().copied() {
        let content_type = match filename.rsplit('.').next() {
            Some("jpg") => "image/jpeg",
            Some("webp") => "image/webp",
            _ => "image/png",
        };
        let (status, body) = upload(
            app.clone(),
            public_id,
            cookie,
            Some(csrf),
            filename,
            content_type,
            bytes,
        )
        .await;
        let actual = body["error"]["fields"]["images"].as_str();
        if status != StatusCode::BAD_REQUEST || actual != Some(expected) {
            failures.push(format!(
                "{filename}: status={status}, reason={actual:?}, expected={expected}, body={body}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM listing_images")
        .fetch_one(pool)
        .await
        .expect("image rows");
    assert_eq!(rows, 0, "rejected input must not create an image row");
    let top_level_files = std::fs::read_dir(dir)
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(top_level_files, 0, "rejected input must not write media");
}

#[tokio::test]
async fn upload_rejects_signature_garbage_and_truncated_real_images() {
    let _lock = UPLOAD_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let dir = scratch_media_dir("media-corrupt");
    let cfg = media_cfg(&dir);
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let public_id = create_draft(&app, &cookie, &csrf).await;
    let garbage = vec![0xFF, 0xD8, 0xFF, b'!', b'!', b'!'];
    let png = png_bytes();
    let webp = webp_bytes();
    assert_image_rejections(
        app,
        &pool,
        &dir,
        &public_id,
        &cookie,
        &csrf,
        &[
            ("garbage.jpg", &garbage, "invalid_image"),
            ("truncated.png", &png[..png.len() / 2], "invalid_image"),
            ("truncated.webp", &webp[..webp.len() - 8], "invalid_image"),
        ],
    )
    .await;
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn upload_rejects_animated_png_and_webp() {
    let _lock = UPLOAD_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let dir = scratch_media_dir("media-animated");
    let cfg = media_cfg(&dir);
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let public_id = create_draft(&app, &cookie, &csrf).await;
    assert_image_rejections(
        app,
        &pool,
        &dir,
        &public_id,
        &cookie,
        &csrf,
        &[
            (
                "animated.png",
                include_bytes!("fixtures/animated.png"),
                "animated",
            ),
            (
                "animated.webp",
                include_bytes!("fixtures/animated.webp"),
                "animated",
            ),
        ],
    )
    .await;
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn upload_rejects_excess_dimensions_and_decoded_pixels() {
    let _lock = UPLOAD_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let dir = scratch_media_dir("media-dimensions");
    let cfg = media_cfg(&dir);
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let public_id = create_draft(&app, &cookie, &csrf).await;
    assert_image_rejections(
        app,
        &pool,
        &dir,
        &public_id,
        &cookie,
        &csrf,
        &[
            (
                "dimensions.png",
                include_bytes!("fixtures/dimensions.png"),
                "too_large",
            ),
            (
                "pixels.png",
                include_bytes!("fixtures/pixels.png"),
                "too_large",
            ),
        ],
    )
    .await;
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn upload_enforces_size_and_count_limits() {
    let _lock = UPLOAD_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let dir = scratch_media_dir("media-limits");
    let mut cfg = media_cfg(&dir);
    cfg.max_image_bytes = 512;
    cfg.max_images_per_listing = 2;
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let public_id = create_draft(&app, &cookie, &csrf).await;

    // 513 bytes > 512-byte limit → 400 before decode or storage.
    let mut big = png_bytes();
    big.resize(513, 0);
    let (status, body) = upload(
        app.clone(),
        &public_id,
        &cookie,
        Some(&csrf),
        "a.png",
        "image/png",
        &big,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["fields"]["images"], "too_large");

    // Two small uploads fill the quota; the third → 400.
    for name in ["one.png", "two.png"] {
        let (status, _) = upload(
            app.clone(),
            &public_id,
            &cookie,
            Some(&csrf),
            name,
            "image/png",
            &png_bytes(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
    let (status, body) = upload(
        app,
        &public_id,
        &cookie,
        Some(&csrf),
        "three.png",
        "image/png",
        &png_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "validation_error");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn reorder_with_foreign_id_is_rejected_and_delete_removes_file() {
    let _lock = UPLOAD_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let dir = scratch_media_dir("media-order");
    let cfg = media_cfg(&dir);
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let public_id = create_draft(&app, &cookie, &csrf).await;

    let (_, first) = upload(
        app.clone(),
        &public_id,
        &cookie,
        Some(&csrf),
        "one.png",
        "image/png",
        &png_bytes(),
    )
    .await;
    let id1: u64 = first["id"].as_u64().expect("image id");
    let (_, second) = upload(
        app.clone(),
        &public_id,
        &cookie,
        Some(&csrf),
        "two.jpg",
        "image/jpeg",
        &jpeg_bytes(),
    )
    .await;
    let id2: u64 = second["id"].as_u64().expect("image id");

    // Reorder with a foreign id mixed in → 400.
    let req = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/api/v1/listings/{public_id}/images/order"))
        .header("content-type", "application/json")
        .header("cookie", &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
            serde_json::json!({ "image_ids": [id2, 999_999] }).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Valid reorder → 200, then delete removes the row AND the file.
    let key2 = second["storage_key"].as_str().unwrap().to_string();
    let req = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/api/v1/listings/{public_id}/images/order"))
        .header("content-type", "application/json")
        .header("cookie", &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
            serde_json::json!({ "image_ids": [id2, id1] }).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let req = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/api/v1/listings/{public_id}/images/{id2}"))
        .header("cookie", &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(!dir.join(&key2).exists(), "delete must remove the file too");
    let _ = std::fs::remove_dir_all(dir);
}
