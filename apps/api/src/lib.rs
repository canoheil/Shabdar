//! Library surface of the Shabdar API: the application itself. main.rs only
//! wires the process; integration tests in tests/ import from here.

pub mod config;
pub mod db;
pub mod error;
pub mod otp;
pub mod otp_service;
pub mod phone;
pub mod request_id;
pub mod routes;
pub mod session;
pub mod state;
