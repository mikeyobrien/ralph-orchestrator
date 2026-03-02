use std::sync::Arc;

use anyhow::Result;
use axum::http::{HeaderMap, header};

use crate::config::{ApiConfig, AuthMode};
use crate::errors::ApiError;
use crate::protocol::RpcRequestEnvelope;

pub trait Authenticator: Send + Sync {
    fn authorize(
        &self,
        request: &RpcRequestEnvelope,
        headers: &HeaderMap,
    ) -> Result<String, ApiError>;
    fn mode(&self) -> AuthMode;
}

#[derive(Debug, Clone, Default)]
pub struct TrustedLocalAuthenticator;

impl Authenticator for TrustedLocalAuthenticator {
    fn authorize(
        &self,
        _request: &RpcRequestEnvelope,
        _headers: &HeaderMap,
    ) -> Result<String, ApiError> {
        Ok("trusted_local".to_string())
    }

    fn mode(&self) -> AuthMode {
        AuthMode::TrustedLocal
    }
}

#[derive(Debug, Clone)]
pub struct TokenAuthenticator {
    expected_token: String,
}

impl TokenAuthenticator {
    pub fn new(expected_token: String) -> Self {
        Self { expected_token }
    }
}

impl Authenticator for TokenAuthenticator {
    fn authorize(
        &self,
        request: &RpcRequestEnvelope,
        headers: &HeaderMap,
    ) -> Result<String, ApiError> {
        let provided_token = token_from_header(headers).or_else(|| {
            request
                .meta
                .as_ref()
                .and_then(|meta| meta.auth.as_ref())
                .and_then(|auth| {
                    if auth.mode == "token" {
                        auth.token.clone()
                    } else {
                        None
                    }
                })
        });

        match provided_token {
            Some(token) if token == self.expected_token => Ok(token),
            Some(_) => Err(ApiError::unauthorized("invalid token")),
            None => Err(ApiError::unauthorized(
                "token auth is enabled and no token was provided",
            )),
        }
    }

    fn mode(&self) -> AuthMode {
        AuthMode::Token
    }
}

pub fn from_config(config: &ApiConfig) -> Result<Arc<dyn Authenticator>> {
    match config.auth_mode {
        AuthMode::TrustedLocal => Ok(Arc::new(TrustedLocalAuthenticator)),
        AuthMode::Token => {
            let token = config
                .token
                .clone()
                .filter(|token| !token.trim().is_empty())
                .ok_or_else(|| anyhow::anyhow!("token auth mode requires RALPH_API_TOKEN"))?;
            Ok(Arc::new(TokenAuthenticator::new(token)))
        }
    }
}

fn token_from_header(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(header::AUTHORIZATION)?;
    let token = raw.to_str().ok()?;
    let token = token.trim();
    token
        .strip_prefix("Bearer ")
        .or_else(|| token.strip_prefix("bearer "))
        .map(std::string::ToString::to_string)
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderMap;
    use serde_json::json;

    use super::{Authenticator, TokenAuthenticator, from_config};
    use crate::config::{ApiConfig, AuthMode};
    use crate::protocol::parse_request;

    #[test]
    fn token_auth_allows_meta_token() {
        let request = parse_request(&json!({
            "apiVersion": "v1",
            "id": "req-1",
            "method": "system.health",
            "params": {},
            "meta": {
                "auth": {
                    "mode": "token",
                    "token": "secret"
                }
            }
        }))
        .expect("request should parse");

        let auth = TokenAuthenticator::new("secret".to_string());
        assert!(auth.authorize(&request, &HeaderMap::new()).is_ok());
    }

    #[test]
    fn from_config_requires_token_for_token_mode() {
        let mut config = ApiConfig::default();
        config.auth_mode = AuthMode::Token;
        config.token = None;

        let result = from_config(&config);
        assert!(result.is_err());
    }
}
