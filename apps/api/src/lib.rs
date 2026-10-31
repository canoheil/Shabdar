//! Library surface of the Shabdar API: the application itself. main.rs only
//! wires the process; integration tests in tests/ import from here.

pub mod admin;
pub mod admin_reports;
pub mod admin_users;
pub mod api_json;
pub mod blocks;
pub mod client_ip;
pub mod comments;
pub mod config;
pub mod csrf;
pub mod db;
pub mod error;
pub mod favorites;
pub mod listing_images;
pub mod listings;
pub mod moderation;
pub mod otp;
pub mod otp_service;
pub mod phone;
pub mod ratelimit;
pub mod ratings;
pub mod reports;
pub mod request_id;
pub mod routes;
pub mod session;
pub mod state;
pub mod user_service;
