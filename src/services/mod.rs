pub mod account;
pub mod order;
pub mod store;
pub mod types;

use std::collections::HashMap;

use failure::{Context, Error as FailureError, Fail};
use hyper::StatusCode;
use serde_json::{self, Value};
use validator::{ValidationError, ValidationErrors};

use stq_api::errors::{Error as ApiError, ErrorMessage as ApiErrorMessage};
use stq_http::client::Error as HttpError;
use stq_http::errors::ErrorMessage as HttpErrorMessage;

use errors::Error;

pub fn parse_validation_errors(e: FailureError, errors: &'static [&str]) -> FailureError {
    {
        let real_err = e.iter_chain().filter_map(CommonErrorMessage::from_fail).nth(0);

        if let Some(CommonErrorMessage {
            payload,
            code,
            description,
        }) = real_err
        {
            match code {
                x if x == StatusCode::Forbidden.as_u16() => return format_err!("{}", description).context(Error::Forbidden).into(),
                x if x == StatusCode::NotFound.as_u16() => return format_err!("{}", description).context(Error::NotFound).into(),
                x if x == StatusCode::BadRequest.as_u16() => {
                    if let Some(payload) = payload {
                        // Weird construction of ValidationErrors due to the fact ValidationErrors.add
                        // only accepts str with static lifetime
                        let valid_err_res = serde_json::from_value::<HashMap<String, Vec<ValidationError>>>(payload.clone());
                        match valid_err_res {
                            Ok(valid_err_map) => {
                                let mut valid_errors = ValidationErrors::new();
                                for error in errors {
                                    if let Some(map_val) = valid_err_map.get(&error.to_string()) {
                                        if !map_val.is_empty() {
                                            valid_errors.add(&error, map_val[0].clone())
                                        }
                                    }
                                }
                                return Error::Validate(valid_errors).into();
                            }
                            Err(e) => {
                                return e.context("Cannot parse validation errors").context(Error::Unknown).into();
                            }
                        }
                    } else {
                        return format_err!("{}", description).context(Error::Unknown).into();
                    }
                }
                _ => return format_err!("{}", description).context(Error::Unknown).into(),
            }
        }
    }
    e
}

struct CommonErrorMessage {
    code: u16,
    description: String,
    payload: Option<Value>,
}

impl CommonErrorMessage {
    fn from_fail(fail: &Fail) -> Option<CommonErrorMessage> {
        if let Some(HttpError::Api(
            _,
            Some(HttpErrorMessage {
                payload,
                code,
                description,
            }),
        )) = fail
            .downcast_ref::<Context<HttpError>>()
            .map(|ctx| ctx.get_context())
            .or(fail.downcast_ref::<HttpError>())
        {
            return Some(CommonErrorMessage {
                code: *code,
                description: description.clone(),
                payload: payload.clone(),
            });
        }

        if let Some(ApiError::Api(
            _,
            Some(ApiErrorMessage {
                payload,
                code,
                description,
            }),
        )) = fail
            .downcast_ref::<Context<ApiError>>()
            .map(|ctx| ctx.get_context())
            .or(fail.downcast_ref::<ApiError>())
        {
            return Some(CommonErrorMessage {
                code: *code,
                description: description.clone(),
                payload: payload.clone(),
            });
        }

        None
    }
}
