use hyper::StatusCode;
use serde_json;
use validator::ValidationErrors;

use stq_api::errors::Error as ApiError;
use stq_http::errors::{Codeable, PayloadCarrier};

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Not found")]
    NotFound,
    #[fail(display = "Parse error")]
    Parse,
    #[fail(display = "Validation error")]
    Validate(ValidationErrors),
    #[fail(display = "Http client error")]
    HttpClient,
    #[fail(display = "Rpc client error")]
    RpcClient,
    #[fail(display = "Server is refusing to fullfil the reqeust")]
    Forbidden,
    #[fail(display = "Unknown server error")]
    Unknown,
}

impl From<ApiError> for Error {
    fn from(api_error: ApiError) -> Error {
        match api_error {
            ApiError::Api(status_code, ref _err_msg) if status_code.as_u16() == StatusCode::Forbidden.as_u16() => Error::Forbidden,
            _ => Error::RpcClient,
        }
    }
}

impl Codeable for Error {
    fn code(&self) -> StatusCode {
        match *self {
            Error::NotFound => StatusCode::NotFound,
            Error::Validate(_) => StatusCode::BadRequest,
            Error::Parse => StatusCode::UnprocessableEntity,
            Error::HttpClient | Error::RpcClient | Error::Unknown => StatusCode::InternalServerError,
            Error::Forbidden => StatusCode::Forbidden,
        }
    }
}

impl PayloadCarrier for Error {
    fn payload(&self) -> Option<serde_json::Value> {
        match *self {
            Error::Validate(ref e) => serde_json::to_value(e.clone()).ok(),
            _ => None,
        }
    }
}
