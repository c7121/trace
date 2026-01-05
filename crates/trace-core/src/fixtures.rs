use uuid::Uuid;

/// Stable dataset UUID for the Query Service fixture dataset.
///
/// Harness code may grant this dataset to all tasks and seed a deterministic Parquet+manifest
/// fixture in MinIO to keep integration tests stable.
pub const ALERTS_FIXTURE_DATASET_ID: Uuid = Uuid::from_bytes([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02,
]);

/// Stable dataset version UUID for the Query Service fixture dataset.
pub const ALERTS_FIXTURE_DATASET_VERSION: Uuid = Uuid::from_bytes([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03,
]);

/// Version-addressed storage prefix (S3 URI) for the alerts fixture dataset manifest + Parquet.
///
/// `crates/trace-query-service` uses this to fetch:
/// - `<prefix>/_manifest.json`
/// - one or more `*.parquet` objects listed by the manifest
pub const ALERTS_FIXTURE_DATASET_STORAGE_PREFIX: &str =
    "s3://trace-harness/cold/datasets/00000000-0000-0000-0000-000000000002/00000000-0000-0000-0000-000000000003/";
