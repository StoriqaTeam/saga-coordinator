pub mod account;
pub mod order;
pub mod store;
pub mod types;

use std::collections::HashMap;

use failure::{Context, Error as FailureError, Fail};
use hyper::StatusCode;
use serde_json;
use validator::{ValidationError, ValidationErrors};

use stq_http::client::Error as HttpError;
use stq_http::errors::ErrorMessage;

use errors::Error;

pub fn parse_validation_errors(e: FailureError, errors: &'static [&str]) -> FailureError {
    {
        let real_err = e
            .causes()
            .filter_map(|cause| {
                if let Some(ctx) = cause.downcast_ref::<Context<Error>>() {
                    Some(ctx.get_context())
                } else {
                    cause.downcast_ref::<Error>()
                }
            })
            .nth(0);
        if let Some(Error::HttpClient(HttpError::Api(
            _,
            Some(ErrorMessage {
                payload,
                code,
                description,
            }),
        ))) = real_err
        {
            match code {
                x if x == &StatusCode::Forbidden.as_u16() => return format_err!("{}", description).context(Error::Forbidden).into(),
                x if x == &StatusCode::NotFound.as_u16() => return format_err!("{}", description).context(Error::NotFound).into(),
                x if x == &StatusCode::BadRequest.as_u16() => {
                    if let Some(payload) = payload {
                        // Wierd construction of ValidationErrors due to the fact ValidationErrors.add
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
