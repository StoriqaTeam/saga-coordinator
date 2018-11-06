use failure::Error;
use futures::prelude::*;
use hyper;
use hyper::header::{Authorization, Headers};
use serde::de::Deserialize;
use serde_json;

use stq_http::client::ClientHandle;

pub struct Response(String);

pub trait HttpClient: Send {
    fn cloned(&self) -> Box<HttpClient>;

    fn request(
        &self,
        method: hyper::Method,
        url: String,
        body: Option<String>,
        headers: Option<Headers>,
    ) -> Box<Future<Item = Response, Error = Error> + Send>;

    fn get(&self, url: String, headers: Option<Headers>) -> Box<Future<Item = Response, Error = Error> + Send> {
        self.request(hyper::Method::Get, url, None, headers)
    }

    fn post(&self, url: String, body: Option<String>, headers: Option<Headers>) -> Box<Future<Item = Response, Error = Error> + Send> {
        self.request(hyper::Method::Post, url, body, headers)
    }

    fn superadmin(&self) -> Box<HttpClient> {
        let mut headers = Headers::new();
        headers.set(Authorization("1".to_string()));
        Box::new(HttpClientWithDefaultHeaders {
            inner: self.cloned(),
            headers,
        })
    }
}

pub struct HttpClientWithDefaultHeaders<S: HttpClient> {
    inner: S,
    headers: Headers,
}

impl Response {
    pub fn parse<T: for<'a> Deserialize<'a> + 'static + Send>(&self) -> Result<T, Error> {
        let response = &self.0;
        if response.is_empty() {
            serde_json::from_value(serde_json::Value::Null)
        } else {
            serde_json::from_str::<T>(&response)
        }.map_err(From::from)
    }
}

impl<S: HttpClient> HttpClientWithDefaultHeaders<S> {
    pub fn new(client: S, headers: Headers) -> Self {
        Self { inner: client, headers }
    }
}

impl<S: HttpClient> HttpClient for HttpClientWithDefaultHeaders<S> {
    fn request(
        &self,
        method: hyper::Method,
        url: String,
        body: Option<String>,
        headers: Option<Headers>,
    ) -> Box<Future<Item = Response, Error = Error> + Send> {
        let headers = if let Some(headers) = headers {
            let mut existing_headers = self.headers.clone();
            existing_headers.extend(headers.iter());
            Some(existing_headers)
        } else {
            Some(self.headers.clone())
        };
        let request = self.inner.request(method, url, body, headers);
        Box::new(request)
    }

    fn cloned(&self) -> Box<HttpClient> {
        Box::new(HttpClientWithDefaultHeaders {
            inner: self.inner.cloned(),
            headers: self.headers.clone(),
        })
    }
}

impl HttpClient for ClientHandle {
    fn request(
        &self,
        method: hyper::Method,
        url: String,
        body: Option<String>,
        headers: Option<Headers>,
    ) -> Box<Future<Item = Response, Error = Error> + Send> {
        Box::new(self.simple_request(method, url, body, headers).map(Response).map_err(From::from))
    }

    fn cloned(&self) -> Box<HttpClient> {
        Box::new(Clone::clone(self))
    }
}

impl HttpClient for Box<dyn HttpClient> {
    fn request(
        &self,
        method: hyper::Method,
        url: String,
        body: Option<String>,
        headers: Option<Headers>,
    ) -> Box<Future<Item = Response, Error = Error> + Send> {
        (**self).request(method, url, body, headers)
    }

    fn cloned(&self) -> Box<HttpClient> {
        (**self).cloned()
    }
}
