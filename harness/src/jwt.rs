use crate::config::HarnessConfig;
use anyhow::{anyhow, Context};
use chrono::Utc;
use jsonwebtoken::{
    decode, decode_header, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone)]
pub struct TaskCapability {
    issuer: String,
    audience: String,
    current_kid: String,
    next_kid: Option<String>,
    ttl: Duration,
    org_id: Uuid,
    current_encoding_key: EncodingKey,
    current_decoding_key: DecodingKey,
    next_decoding_key: Option<DecodingKey>,
}

impl TaskCapability {
    pub fn from_config(cfg: &HarnessConfig) -> anyhow::Result<Self> {
        let secret = cfg.task_capability_secret.as_bytes();
        let next_kid = cfg.task_capability_next_kid.clone();
        let next_secret = cfg.task_capability_next_secret.clone();
        if next_kid.is_some() != next_secret.is_some() {
            return Err(anyhow!(
                "TASK_CAPABILITY_NEXT_KID and TASK_CAPABILITY_NEXT_SECRET must be set together"
            ));
        }

        Ok(Self {
            issuer: cfg.task_capability_iss.clone(),
            audience: cfg.task_capability_aud.clone(),
            current_kid: cfg.task_capability_kid.clone(),
            next_kid,
            ttl: Duration::from_secs(cfg.task_capability_ttl_secs),
            org_id: cfg.org_id,
            current_encoding_key: EncodingKey::from_secret(secret),
            current_decoding_key: DecodingKey::from_secret(secret),
            next_decoding_key: next_secret
                .as_deref()
                .map(|s| DecodingKey::from_secret(s.as_bytes())),
        })
    }

    pub fn issue(&self, task_id: Uuid, attempt: i64) -> anyhow::Result<String> {
        let now = Utc::now().timestamp();
        let iat: usize = now.try_into().unwrap_or(0);
        let exp: usize = (now + self.ttl.as_secs().try_into().unwrap_or(i64::MAX))
            .try_into()
            .unwrap_or(usize::MAX);

        let claims = TaskCapabilityClaims {
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
            sub: format!("task:{task_id}"),
            exp,
            iat,
            org_id: self.org_id,
            task_id,
            attempt,
            datasets: Vec::new(),
            s3: S3Grants::empty(),
        };

        let mut header = Header::new(Algorithm::HS256);
        header.kid = Some(self.current_kid.clone());
        encode(&header, &claims, &self.current_encoding_key).context("encode task capability token")
    }

    pub fn verify(&self, token: &str) -> anyhow::Result<TaskCapabilityClaims> {
        let header = decode_header(token).context("decode jwt header")?;
        let kid = header
            .kid
            .as_deref()
            .ok_or_else(|| anyhow!("missing jwt kid"))?;

        let decoding_key = if kid == self.current_kid {
            &self.current_decoding_key
        } else if self.next_kid.as_deref() == Some(kid) {
            self.next_decoding_key
                .as_ref()
                .ok_or_else(|| anyhow!("next jwt key not configured"))?
        } else {
            return Err(anyhow!("invalid jwt kid"));
        };

        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(std::slice::from_ref(&self.issuer));
        validation.set_audience(std::slice::from_ref(&self.audience));

        let data = decode::<TaskCapabilityClaims>(token, decoding_key, &validation)
            .context("verify jwt")?;
        Ok(data.claims)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCapabilityClaims {
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub exp: usize,
    pub iat: usize,

    pub org_id: Uuid,
    pub task_id: Uuid,
    pub attempt: i64,
    pub datasets: Vec<DatasetGrant>,
    pub s3: S3Grants,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetGrant {
    pub dataset_uuid: Uuid,
    pub dataset_version: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Grants {
    pub read_prefixes: Vec<String>,
    pub write_prefixes: Vec<String>,
}

impl S3Grants {
    fn empty() -> Self {
        Self {
            read_prefixes: Vec::new(),
            write_prefixes: Vec::new(),
        }
    }
}
