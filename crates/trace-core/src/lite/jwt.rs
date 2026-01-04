use crate::{Error, Result, Signer, TaskCapabilityClaims, TaskCapabilityIssueRequest};
use anyhow::Context;
use chrono::Utc;
use jsonwebtoken::{
    decode, decode_header, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation,
};
use std::time::Duration;

#[derive(Clone)]
pub struct Hs256TaskCapabilityConfig {
    pub issuer: String,
    pub audience: String,
    pub current_kid: String,
    pub current_secret: String,
    pub next_kid: Option<String>,
    pub next_secret: Option<String>,
    pub ttl: Duration,
}

impl std::fmt::Debug for Hs256TaskCapabilityConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let next_secret = self.next_secret.as_deref().map(|_| "<redacted>");
        f.debug_struct("Hs256TaskCapabilityConfig")
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .field("current_kid", &self.current_kid)
            .field("current_secret", &"<redacted>")
            .field("next_kid", &self.next_kid)
            .field("next_secret", &next_secret)
            .field("ttl", &self.ttl)
            .finish()
    }
}

#[derive(Clone)]
pub struct TaskCapability {
    issuer: String,
    audience: String,
    current_kid: String,
    next_kid: Option<String>,
    ttl: Duration,
    current_encoding_key: EncodingKey,
    current_decoding_key: DecodingKey,
    next_decoding_key: Option<DecodingKey>,
}

impl std::fmt::Debug for TaskCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let next_decoding_key = self.next_decoding_key.as_ref().map(|_| "<redacted>");
        f.debug_struct("TaskCapability")
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .field("current_kid", &self.current_kid)
            .field("next_kid", &self.next_kid)
            .field("ttl", &self.ttl)
            .field("current_encoding_key", &"<redacted>")
            .field("current_decoding_key", &"<redacted>")
            .field("next_decoding_key", &next_decoding_key)
            .finish()
    }
}

impl TaskCapability {
    pub fn from_hs256_config(cfg: Hs256TaskCapabilityConfig) -> Result<Self> {
        if cfg.next_kid.is_some() != cfg.next_secret.is_some() {
            return Err(Error::msg("next_kid and next_secret must be set together"));
        }

        let secret = cfg.current_secret.as_bytes();
        Ok(Self {
            issuer: cfg.issuer,
            audience: cfg.audience,
            current_kid: cfg.current_kid,
            next_kid: cfg.next_kid,
            ttl: cfg.ttl,
            current_encoding_key: EncodingKey::from_secret(secret),
            current_decoding_key: DecodingKey::from_secret(secret),
            next_decoding_key: cfg
                .next_secret
                .as_deref()
                .map(|s| DecodingKey::from_secret(s.as_bytes())),
        })
    }

    pub fn issue(&self, req: &TaskCapabilityIssueRequest) -> Result<String> {
        let now = Utc::now().timestamp();
        let iat: usize = now.try_into().unwrap_or(0);
        let exp: usize = (now + self.ttl.as_secs().try_into().unwrap_or(i64::MAX))
            .try_into()
            .unwrap_or(usize::MAX);

        let task_id = req.task_id;
        let claims = TaskCapabilityClaims {
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
            sub: format!("task:{task_id}"),
            exp,
            iat,
            org_id: req.org_id,
            task_id: req.task_id,
            attempt: req.attempt,
            datasets: req.datasets.clone(),
            s3: req.s3.clone(),
        };

        let mut header = Header::new(Algorithm::HS256);
        header.kid = Some(self.current_kid.clone());
        encode(&header, &claims, &self.current_encoding_key)
            .context("encode task capability token")
            .map_err(Error::from)
    }

    pub fn verify(&self, token: &str) -> Result<TaskCapabilityClaims> {
        let header = decode_header(token)
            .context("decode jwt header")
            .map_err(Error::from)?;
        let kid = header
            .kid
            .as_deref()
            .ok_or_else(|| Error::msg("missing jwt kid"))?;

        let decoding_key = if kid == self.current_kid {
            &self.current_decoding_key
        } else if self.next_kid.as_deref() == Some(kid) {
            self.next_decoding_key
                .as_ref()
                .ok_or_else(|| Error::msg("next jwt key not configured"))?
        } else {
            return Err(Error::msg("invalid jwt kid"));
        };

        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(std::slice::from_ref(&self.issuer));
        validation.set_audience(std::slice::from_ref(&self.audience));

        let data = decode::<TaskCapabilityClaims>(token, decoding_key, &validation)
            .context("verify jwt")
            .map_err(Error::from)?;
        Ok(data.claims)
    }
}

impl Signer for TaskCapability {
    fn issue_task_capability(&self, req: &TaskCapabilityIssueRequest) -> Result<String> {
        self.issue(req)
    }

    fn verify_task_capability(&self, token: &str) -> Result<TaskCapabilityClaims> {
        self.verify(token)
    }
}
