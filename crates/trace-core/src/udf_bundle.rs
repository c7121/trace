use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fmt,
    path::{Component, Path},
};

pub const BUNDLE_MANIFEST_FILE: &str = "bundle_manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UdfBundleManifestV1 {
    pub schema_version: u32,
    pub runtime: UdfBundleRuntime,
    pub entrypoint: String,
    pub files: Vec<UdfBundleFile>,

    #[serde(default)]
    pub env_allowlist: Vec<String>,
}

impl UdfBundleManifestV1 {
    pub const SCHEMA_VERSION: u32 = 1;

    pub fn validate(&self, limits: &UdfBundleLimits) -> Result<(), UdfBundleError> {
        if self.schema_version != Self::SCHEMA_VERSION {
            return Err(UdfBundleError::permanent(format!(
                "unsupported schema_version {}",
                self.schema_version
            )));
        }

        validate_bundle_relpath(&self.entrypoint)
            .map_err(|e| e.with_context("invalid entrypoint"))?;

        if self.files.len() > limits.max_files {
            return Err(UdfBundleError::permanent(format!(
                "too many files: {} > {}",
                self.files.len(),
                limits.max_files
            )));
        }

        let mut total_bytes: u64 = 0;
        let mut paths = HashSet::<&str>::new();
        let mut entrypoint_present = false;

        for file in &self.files {
            if file.path == self.entrypoint {
                entrypoint_present = true;
            }

            if !paths.insert(file.path.as_str()) {
                return Err(UdfBundleError::permanent(format!(
                    "duplicate file path '{}'",
                    file.path
                )));
            }

            validate_bundle_relpath(&file.path)
                .map_err(|e| e.with_context(format!("invalid file path '{}'", file.path)))?;

            if file.path == BUNDLE_MANIFEST_FILE {
                return Err(UdfBundleError::permanent(format!(
                    "file path '{}' is reserved",
                    BUNDLE_MANIFEST_FILE
                )));
            }

            if file.bytes > limits.max_file_bytes {
                return Err(UdfBundleError::permanent(format!(
                    "file '{}' too large: {} > {} bytes",
                    file.path, file.bytes, limits.max_file_bytes
                )));
            }

            total_bytes = total_bytes
                .checked_add(file.bytes)
                .ok_or_else(|| UdfBundleError::permanent("total bytes overflow"))?;

            if total_bytes > limits.max_total_uncompressed_bytes {
                return Err(UdfBundleError::permanent(format!(
                    "total uncompressed bytes too large: {} > {}",
                    total_bytes, limits.max_total_uncompressed_bytes
                )));
            }

            validate_sha256_hex(&file.sha256)
                .map_err(|e| e.with_context(format!("invalid sha256 for '{}'", file.path)))?;
        }

        if !entrypoint_present {
            return Err(UdfBundleError::permanent(format!(
                "entrypoint '{}' is not declared in files",
                self.entrypoint
            )));
        }

        for env_name in &self.env_allowlist {
            if !is_safe_env_var_name(env_name) {
                return Err(UdfBundleError::permanent(format!(
                    "invalid env var name '{}'",
                    env_name
                )));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UdfBundleRuntime {
    Node,
    Python,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UdfBundleFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone)]
pub struct UdfBundleLimits {
    pub max_files: usize,
    pub max_file_bytes: u64,
    pub max_total_uncompressed_bytes: u64,
}

impl Default for UdfBundleLimits {
    fn default() -> Self {
        Self {
            max_files: 1_000,
            max_file_bytes: 32 * 1024 * 1024,
            max_total_uncompressed_bytes: 256 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UdfBundleErrorKind {
    Permanent,
    Retryable,
}

#[derive(Debug)]
pub struct UdfBundleError {
    pub kind: UdfBundleErrorKind,
    message: String,
}

impl UdfBundleError {
    pub fn permanent(message: impl Into<String>) -> Self {
        Self {
            kind: UdfBundleErrorKind::Permanent,
            message: message.into(),
        }
    }

    pub fn retryable(message: impl Into<String>) -> Self {
        Self {
            kind: UdfBundleErrorKind::Retryable,
            message: message.into(),
        }
    }

    fn with_context(self, context: impl Into<String>) -> Self {
        let ctx = context.into();
        match self.kind {
            UdfBundleErrorKind::Permanent => {
                UdfBundleError::permanent(format!("{ctx}: {}", self.message))
            }
            UdfBundleErrorKind::Retryable => {
                UdfBundleError::retryable(format!("{ctx}: {}", self.message))
            }
        }
    }
}

impl fmt::Display for UdfBundleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for UdfBundleError {}

fn validate_bundle_relpath(path: &str) -> Result<(), UdfBundleError> {
    if path.is_empty() {
        return Err(UdfBundleError::permanent("path must not be empty"));
    }
    if path.contains('\0') {
        return Err(UdfBundleError::permanent("path must not contain NUL"));
    }
    if path.contains('\\') {
        return Err(UdfBundleError::permanent(
            "path must not contain backslashes",
        ));
    }
    if path.starts_with('/') {
        return Err(UdfBundleError::permanent("path must not be absolute"));
    }
    if path.ends_with('/') {
        return Err(UdfBundleError::permanent("path must not end with '/'"));
    }

    let p = Path::new(path);
    for c in p.components() {
        match c {
            Component::Normal(_) => {}
            Component::CurDir => {
                return Err(UdfBundleError::permanent(
                    "path must not contain '.' segments",
                ));
            }
            Component::ParentDir => {
                return Err(UdfBundleError::permanent(
                    "path must not contain '..' segments",
                ));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(UdfBundleError::permanent("path must not be absolute"));
            }
        }
    }
    Ok(())
}

fn validate_sha256_hex(s: &str) -> Result<(), UdfBundleError> {
    if s.len() != 64 {
        return Err(UdfBundleError::permanent("sha256 must be 64 hex chars"));
    }
    if !s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) {
        return Err(UdfBundleError::permanent("sha256 must be lowercase hex"));
    }
    Ok(())
}

fn is_safe_env_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !matches!(first, 'A'..='Z' | '_') {
        return false;
    }
    chars.all(|c| matches!(c, 'A'..='Z' | '0'..='9' | '_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_manifest() -> UdfBundleManifestV1 {
        UdfBundleManifestV1 {
            schema_version: UdfBundleManifestV1::SCHEMA_VERSION,
            runtime: UdfBundleRuntime::Node,
            entrypoint: "index.js".to_string(),
            files: vec![UdfBundleFile {
                path: "index.js".to_string(),
                sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                    .to_string(),
                bytes: 0,
            }],
            env_allowlist: vec![],
        }
    }

    #[test]
    fn validate_accepts_minimal_manifest() {
        let m = minimal_manifest();
        m.validate(&UdfBundleLimits::default()).unwrap();
    }

    #[test]
    fn validate_rejects_entrypoint_not_in_files() {
        let mut m = minimal_manifest();
        m.entrypoint = "main.js".to_string();
        let err = m.validate(&UdfBundleLimits::default()).unwrap_err();
        assert_eq!(err.kind, UdfBundleErrorKind::Permanent);
    }

    #[test]
    fn validate_rejects_parent_dir_paths() {
        let mut m = minimal_manifest();
        m.files[0].path = "../index.js".to_string();
        let err = m.validate(&UdfBundleLimits::default()).unwrap_err();
        assert_eq!(err.kind, UdfBundleErrorKind::Permanent);
    }

    #[test]
    fn validate_rejects_absolute_paths() {
        let mut m = minimal_manifest();
        m.files[0].path = "/index.js".to_string();
        let err = m.validate(&UdfBundleLimits::default()).unwrap_err();
        assert_eq!(err.kind, UdfBundleErrorKind::Permanent);
    }

    #[test]
    fn validate_rejects_backslashes() {
        let mut m = minimal_manifest();
        m.files[0].path = "dir\\index.js".to_string();
        let err = m.validate(&UdfBundleLimits::default()).unwrap_err();
        assert_eq!(err.kind, UdfBundleErrorKind::Permanent);
    }

    #[test]
    fn validate_rejects_invalid_sha256() {
        let mut m = minimal_manifest();
        m.files[0].sha256 = "not-a-sha".to_string();
        let err = m.validate(&UdfBundleLimits::default()).unwrap_err();
        assert_eq!(err.kind, UdfBundleErrorKind::Permanent);
    }

    #[test]
    fn validate_rejects_too_many_files() {
        let mut m = minimal_manifest();
        m.files.push(UdfBundleFile {
            path: "extra.js".to_string(),
            sha256: m.files[0].sha256.clone(),
            bytes: 0,
        });
        let limits = UdfBundleLimits {
            max_files: 1,
            ..UdfBundleLimits::default()
        };
        let err = m.validate(&limits).unwrap_err();
        assert_eq!(err.kind, UdfBundleErrorKind::Permanent);
    }

    #[test]
    fn validate_rejects_total_bytes_over_limit() {
        let mut m = minimal_manifest();
        m.files[0].bytes = 5;
        let limits = UdfBundleLimits {
            max_total_uncompressed_bytes: 4,
            ..UdfBundleLimits::default()
        };
        let err = m.validate(&limits).unwrap_err();
        assert_eq!(err.kind, UdfBundleErrorKind::Permanent);
    }

    #[test]
    fn validate_rejects_invalid_env_var_names() {
        let mut m = minimal_manifest();
        m.env_allowlist = vec!["trace_secret".to_string()];
        let err = m.validate(&UdfBundleLimits::default()).unwrap_err();
        assert_eq!(err.kind, UdfBundleErrorKind::Permanent);
    }
}
