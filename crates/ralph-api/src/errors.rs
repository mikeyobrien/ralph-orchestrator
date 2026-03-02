use axum::http::StatusCode;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcErrorCode {
    InvalidRequest,
    MethodNotFound,
    InvalidParams,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    PreconditionFailed,
    RateLimited,
    Timeout,
    ServiceUnavailable,
    Internal,
    TaskNotFound,
    LoopNotFound,
    PlanningSessionNotFound,
    CollectionNotFound,
    ConfigInvalid,
    IdempotencyConflict,
    BackpressureDropped,
}

impl RpcErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::MethodNotFound => "METHOD_NOT_FOUND",
            Self::InvalidParams => "INVALID_PARAMS",
            Self::Unauthorized => "UNAUTHORIZED",
            Self::Forbidden => "FORBIDDEN",
            Self::NotFound => "NOT_FOUND",
            Self::Conflict => "CONFLICT",
            Self::PreconditionFailed => "PRECONDITION_FAILED",
            Self::RateLimited => "RATE_LIMITED",
            Self::Timeout => "TIMEOUT",
            Self::ServiceUnavailable => "SERVICE_UNAVAILABLE",
            Self::Internal => "INTERNAL",
            Self::TaskNotFound => "TASK_NOT_FOUND",
            Self::LoopNotFound => "LOOP_NOT_FOUND",
            Self::PlanningSessionNotFound => "PLANNING_SESSION_NOT_FOUND",
            Self::CollectionNotFound => "COLLECTION_NOT_FOUND",
            Self::ConfigInvalid => "CONFIG_INVALID",
            Self::IdempotencyConflict => "IDEMPOTENCY_CONFLICT",
            Self::BackpressureDropped => "BACKPRESSURE_DROPPED",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcErrorBody {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct ApiError {
    pub code: RpcErrorCode,
    pub message: String,
    pub retryable: bool,
    pub details: Option<Value>,
    pub status: StatusCode,
    pub request_id: String,
    pub method: Option<String>,
}

impl ApiError {
    pub fn new(code: RpcErrorCode, message: impl Into<String>) -> Self {
        Self {
            status: status_for_code(code),
            code,
            message: message.into(),
            retryable: matches!(
                code,
                RpcErrorCode::RateLimited
                    | RpcErrorCode::Timeout
                    | RpcErrorCode::ServiceUnavailable
                    | RpcErrorCode::BackpressureDropped
            ),
            details: None,
            request_id: "unknown".to_string(),
            method: None,
        }
    }

    pub fn with_context(mut self, request_id: impl Into<String>, method: Option<String>) -> Self {
        self.request_id = request_id.into();
        self.method = method;
        self
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(RpcErrorCode::InvalidRequest, message)
    }

    pub fn method_not_found(method: impl Into<String>) -> Self {
        let method = method.into();
        Self::new(
            RpcErrorCode::MethodNotFound,
            format!("method '{method}' is not supported by rpc v1"),
        )
        .with_details(serde_json::json!({ "method": method }))
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(RpcErrorCode::InvalidParams, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(RpcErrorCode::Unauthorized, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(RpcErrorCode::Forbidden, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(RpcErrorCode::Conflict, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(RpcErrorCode::NotFound, message)
    }

    pub fn precondition_failed(message: impl Into<String>) -> Self {
        Self::new(RpcErrorCode::PreconditionFailed, message)
    }

    pub fn task_not_found(message: impl Into<String>) -> Self {
        Self::new(RpcErrorCode::TaskNotFound, message)
    }

    pub fn loop_not_found(message: impl Into<String>) -> Self {
        Self::new(RpcErrorCode::LoopNotFound, message)
    }

    pub fn planning_session_not_found(message: impl Into<String>) -> Self {
        Self::new(RpcErrorCode::PlanningSessionNotFound, message)
    }

    pub fn collection_not_found(message: impl Into<String>) -> Self {
        Self::new(RpcErrorCode::CollectionNotFound, message)
    }

    pub fn config_invalid(message: impl Into<String>) -> Self {
        Self::new(RpcErrorCode::ConfigInvalid, message)
    }

    pub fn idempotency_conflict(message: impl Into<String>) -> Self {
        Self::new(RpcErrorCode::IdempotencyConflict, message)
    }

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::new(RpcErrorCode::ServiceUnavailable, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(RpcErrorCode::Internal, message)
    }

    pub fn as_body(&self) -> RpcErrorBody {
        RpcErrorBody {
            code: self.code.as_str().to_string(),
            message: self.message.clone(),
            retryable: self.retryable,
            details: self.details.clone(),
        }
    }
}

const fn status_for_code(code: RpcErrorCode) -> StatusCode {
    match code {
        RpcErrorCode::InvalidRequest | RpcErrorCode::InvalidParams => StatusCode::BAD_REQUEST,
        RpcErrorCode::MethodNotFound => StatusCode::NOT_FOUND,
        RpcErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
        RpcErrorCode::Forbidden => StatusCode::FORBIDDEN,
        RpcErrorCode::NotFound
        | RpcErrorCode::TaskNotFound
        | RpcErrorCode::LoopNotFound
        | RpcErrorCode::PlanningSessionNotFound
        | RpcErrorCode::CollectionNotFound => StatusCode::NOT_FOUND,
        RpcErrorCode::Conflict | RpcErrorCode::IdempotencyConflict => StatusCode::CONFLICT,
        RpcErrorCode::PreconditionFailed => StatusCode::PRECONDITION_FAILED,
        RpcErrorCode::RateLimited => StatusCode::TOO_MANY_REQUESTS,
        RpcErrorCode::Timeout => StatusCode::REQUEST_TIMEOUT,
        RpcErrorCode::ServiceUnavailable | RpcErrorCode::BackpressureDropped => {
            StatusCode::SERVICE_UNAVAILABLE
        }
        RpcErrorCode::ConfigInvalid => StatusCode::BAD_REQUEST,
        RpcErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
