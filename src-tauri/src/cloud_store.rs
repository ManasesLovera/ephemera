// Google Cloud Storage via direct REST calls (JSON API), authenticated with a
// self-signed JWT exchanged for an OAuth2 access token. This avoids depending on the
// less-stable `google-cloud-storage` community crate — see docs/09-gcs-tier.md.

use crate::error::{assert_fits, AppError};
use crate::types::{CloudFile, FileId, FileMeta, Origin, MAX_CLOUD_BYTES};
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Deserialize)]
struct ServiceAccountKey {
    client_email: String,
    private_key: String,
    token_uri: String,
}

#[derive(Debug, Serialize)]
struct Claims {
    iss: String,
    scope: String,
    aud: String,
    exp: i64,
    iat: i64,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
}

struct CachedToken {
    token: String,
    fetched_at: Instant,
    ttl: Duration,
}

pub struct CloudStore {
    key: Option<ServiceAccountKey>,
    bucket: String,
    client: reqwest::Client,
    token: RwLock<Option<CachedToken>>,
    offline_reason: Option<String>,
}

fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

impl CloudStore {
    pub fn load(key_path: &str, bucket: String) -> Self {
        match std::fs::read_to_string(key_path) {
            Ok(contents) => match serde_json::from_str::<ServiceAccountKey>(&contents) {
                Ok(key) => Self {
                    key: Some(key),
                    bucket,
                    client: reqwest::Client::new(),
                    token: RwLock::new(None),
                    offline_reason: None,
                },
                Err(e) => Self::offline(bucket, format!("invalid key file: {e}")),
            },
            Err(_) => Self::offline(bucket, "no credentials — see docs/09-gcs-tier.md".to_string()),
        }
    }

    fn offline(bucket: String, reason: String) -> Self {
        Self {
            key: None,
            bucket,
            client: reqwest::Client::new(),
            token: RwLock::new(None),
            offline_reason: Some(reason),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.key.is_some()
    }

    pub fn offline_reason(&self) -> Option<String> {
        self.offline_reason.clone()
    }

    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    async fn access_token(&self) -> Result<String, AppError> {
        if let Some(cached) = self.token.read().unwrap().as_ref() {
            if cached.fetched_at.elapsed() < cached.ttl {
                return Ok(cached.token.clone());
            }
        }
        let key = self.key.as_ref().ok_or_else(|| AppError::CloudUnavailable {
            message: self.offline_reason.clone().unwrap_or_else(|| "not configured".into()),
        })?;

        let now = chrono::Utc::now().timestamp();
        let claims = Claims {
            iss: key.client_email.clone(),
            scope: "https://www.googleapis.com/auth/devstorage.read_write".to_string(),
            aud: key.token_uri.clone(),
            exp: now + 3600,
            iat: now,
        };
        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        let encoding_key = jsonwebtoken::EncodingKey::from_rsa_pem(key.private_key.as_bytes())
            .map_err(|e| AppError::CloudUnavailable { message: format!("bad private key: {e}") })?;
        let jwt = jsonwebtoken::encode(&header, &claims, &encoding_key)
            .map_err(|e| AppError::CloudUnavailable { message: format!("jwt sign failed: {e}") })?;

        let resp = self
            .client
            .post(&key.token_uri)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", jwt.as_str()),
            ])
            .send()
            .await
            .map_err(|e| AppError::CloudUnavailable { message: e.to_string() })?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::CloudUnavailable { message: format!("token exchange failed: {body}") });
        }
        let token_resp: TokenResponse = resp
            .json()
            .await
            .map_err(|e| AppError::CloudUnavailable { message: e.to_string() })?;

        *self.token.write().unwrap() = Some(CachedToken {
            token: token_resp.access_token.clone(),
            fetched_at: Instant::now(),
            ttl: Duration::from_secs((token_resp.expires_in - 60).max(60) as u64),
        });
        Ok(token_resp.access_token)
    }

    pub async fn bytes_used(&self) -> Result<u64, AppError> {
        let objects = self.list_raw().await?;
        Ok(objects.iter().map(|o| o.size.parse::<u64>().unwrap_or(0)).sum())
    }

    async fn list_raw(&self) -> Result<Vec<GcsObject>, AppError> {
        if self.key.is_none() {
            return Ok(vec![]);
        }
        let token = self.access_token().await?;
        let url = format!("https://storage.googleapis.com/storage/v1/b/{}/o", self.bucket);
        let resp = self
            .client
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| AppError::CloudUnavailable { message: e.to_string() })?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(vec![]);
        }
        let listing: GcsListing = resp
            .json()
            .await
            .map_err(|e| AppError::CloudUnavailable { message: e.to_string() })?;
        Ok(listing.items.unwrap_or_default())
    }

    pub async fn list(&self) -> Result<Vec<CloudFile>, AppError> {
        let objects = self.list_raw().await?;
        Ok(objects
            .into_iter()
            .map(|o| CloudFile {
                meta: FileMeta {
                    id: o.name.clone(),
                    name: o.name.clone(),
                    size: o.size.parse().unwrap_or(0),
                    mime: o.content_type.unwrap_or_default(),
                    created_at: now_millis(),
                    origin: Origin::Disk,
                },
                saved_at: now_millis(),
                object_name: o.name,
            })
            .collect())
    }

    pub async fn upload(&self, meta: &FileMeta, bytes: Vec<u8>) -> Result<CloudFile, AppError> {
        let current = self.bytes_used().await.unwrap_or(0);
        assert_fits(current, meta.size, MAX_CLOUD_BYTES)?;
        let token = self.access_token().await?;
        let object_name = format!("{}-{}", meta.id, meta.name);
        let url = format!(
            "https://storage.googleapis.com/upload/storage/v1/b/{}/o?uploadType=media&name={}",
            self.bucket,
            urlencoding::encode(&object_name)
        );
        let resp = self
            .client
            .post(&url)
            .bearer_auth(token)
            .header("Content-Type", meta.mime.clone())
            .body(bytes)
            .send()
            .await
            .map_err(|e| AppError::CloudUnavailable { message: e.to_string() })?;
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::CloudUnavailable { message: format!("upload failed: {body}") });
        }
        Ok(CloudFile { meta: meta.clone(), saved_at: now_millis(), object_name })
    }

    pub async fn remove(&self, object_name: &FileId) -> Result<(), AppError> {
        let token = self.access_token().await?;
        let url = format!(
            "https://storage.googleapis.com/storage/v1/b/{}/o/{}",
            self.bucket,
            urlencoding::encode(object_name)
        );
        self.client
            .delete(&url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| AppError::CloudUnavailable { message: e.to_string() })?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct GcsListing {
    items: Option<Vec<GcsObject>>,
}

#[derive(Debug, Deserialize)]
struct GcsObject {
    name: String,
    size: String,
    #[serde(rename = "contentType")]
    content_type: Option<String>,
}

mod urlencoding {
    pub fn encode(s: &str) -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
                _ => format!("%{:02X}", b),
            })
            .collect()
    }
}
