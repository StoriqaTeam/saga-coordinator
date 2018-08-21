use hyper::StatusCode;
use serde_json;
use validator::ValidationErrors;

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
