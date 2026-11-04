use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Request};
use axum::Json;
use serde::de::DeserializeOwned;

use crate::error::ApiError;

pub const WRONG_CONTENT_TYPE: &str = "expected content-type application/json";
pub const INVALID_JSON: &str = "invalid json body";

/// `Json<T>` whose rejections (not JSON, wrong content type, wrong fields) are a `400`
/// in the usual envelope, in the contract's lowercase copy. Every phase 2 route takes its
/// body through this.
#[derive(Debug, Clone)]
pub struct JsonBody<T>(pub T);

fn deepest(error: &dyn std::error::Error) -> String {
    let mut current = error;
    while let Some(next) = current.source() {
        current = next;
    }
    current.to_string()
}

fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// `expected content-type application/json`, or `invalid json body: <what serde said>`.
pub fn rejection_message(rejection: &JsonRejection) -> String {
    match rejection {
        JsonRejection::MissingJsonContentType(_) => WRONG_CONTENT_TYPE.to_string(),
        other => format!("{INVALID_JSON}: {}", lower_first(&deepest(other))),
    }
}

impl<S, T> FromRequest<S> for JsonBody<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(ApiError::BadRequest(rejection_message(&rejection))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercases_only_the_first_letter() {
        assert_eq!(lower_first("Expected value"), "expected value");
        assert_eq!(
            lower_first("unknown field `baseBranch`"),
            "unknown field `baseBranch`"
        );
        assert_eq!(lower_first(""), "");
    }
}
