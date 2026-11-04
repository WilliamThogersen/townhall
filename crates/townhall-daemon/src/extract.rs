use axum::extract::{FromRequest, Request};
use axum::Json;
use serde::de::DeserializeOwned;

use crate::error::ApiError;

/// `Json<T>` whose rejections (not JSON, wrong content type, wrong fields) are a `400`
/// in the usual envelope. Every phase 2 route takes its body through this.
#[derive(Debug, Clone)]
pub struct JsonBody<T>(pub T);

impl<S, T> FromRequest<S> for JsonBody<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(ApiError::BadRequest(rejection.body_text())),
        }
    }
}
