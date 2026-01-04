//! Lite-mode implementations of `trace-core` interfaces.
//!
//! This module provides a minimal Postgres-backed queue, an S3-compatible object store client,
//! and an HS256 task capability signer for local development and harness flows.

pub mod jwt;
pub mod pgqueue;
pub mod s3;
