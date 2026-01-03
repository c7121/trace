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
    kid: String,
    ttl: Duration,
    org_id: Uuid,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl TaskCapability {
    pub fn from_config(cfg: &HarnessConfig) -> anyhow::Result<Self> {
        let secret = cfg.task_capability_secret.as_bytes();
        Ok(Self {
            issuer: cfg.task_capability_iss.clone(),
            audience: cfg.task_capability_aud.clone(),
            kid: cfg.task_capability_kid.clone(),
            ttl: Duration::from_secs(cfg.task_capability_ttl_secs),
            org_id: cfg.org_id,
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
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
        header.kid = Some(self.kid.clone());
        encode(&header, &claims, &self.encoding_key).context("encode task capability token")
    }

    pub fn verify(&self, token: &str) -> anyhow::Result<TaskCapabilityClaims> {
        let header = decode_header(token).context("decode jwt header")?;
        if header.kid.as_deref() != Some(&self.kid) {
            return Err(anyhow!("invalid jwt kid"));
        }

        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(std::slice::from_ref(&self.issuer));
        validation.set_audience(std::slice::from_ref(&self.audience));

        let data = decode::<TaskCapabilityClaims>(token, &self.decoding_key, &validation)
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
