use failure::Error;
use futures::{Future, IntoFuture};
use hyper::header::Headers;
use hyper::Method;
use serde::de::Deserialize;
use serde::ser::Serialize;
use serde_json;

use http::HttpClient;

mod orders;
pub use self::orders::*;

mod stores;
pub use self::stores::*;

mod notifications;
pub use self::notifications::*;

mod users;
pub use self::users::*;

mod billing;
pub use self::billing::*;

mod warehouses;
pub use self::warehouses::*;

pub type ApiFuture<T> = Box<Future<Item = T, Error = Error> + Send>;

fn request<C: HttpClient + 'static, T: Serialize, S: for<'a> Deserialize<'a> + 'static + Send>(
    http_client: C,
    method: Method,
    url: String,
    payload: Option<T>,
    headers: Option<Headers>,
) -> ApiFuture<S> {
    let body = if let Some(payload) = payload {
        serde_json::to_string::<T>(&payload).map(Some)
    } else {
        Ok(None)
    };

    let result = body
        .into_future()
        .map_err(From::from)
        .and_then(move |serialized_body| http_client.request(method, url, serialized_body, headers))
        .and_then(|response| response.parse::<S>().into_future());
    Box::new(result)
}
