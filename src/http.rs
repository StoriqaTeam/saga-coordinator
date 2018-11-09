use failure::Error;
use futures::prelude::*;
use hyper;
use hyper::header::Headers;
use serde::de::Deserialize;
use serde_json;

use stq_http::client::ClientHandle;

pub struct Response(String);

pub trait HttpClient: Send {
    fn request(
        &self,
        method: hyper::Method,
        url: String,
        body: Option<String>,
        headers: Option<Headers>,
    ) -> Box<Future<Item = Response, Error = Error> + Send>;
}

#[derive(Clone)]
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
}

impl<T: HttpClient> HttpClient for Box<T> {
    fn request(
        &self,
        method: hyper::Method,
        url: String,
        body: Option<String>,
        headers: Option<Headers>,
    ) -> Box<Future<Item = Response, Error = Error> + Send> {
        (**self).request(method, url, body, headers)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use failure::Error;
    use futures::future;
    use futures::prelude::*;
    use hyper;
    use hyper::header::{Authorization, Headers};
    use hyper::Method;

    use super::*;

    #[test]
    fn new_headers_override_existing_headers() {
        //given
        let mock_client = MockHttpClient::new();
        let client_with_old_default_headers = HttpClientWithDefaultHeaders {
            inner: mock_client.clone(),
            headers: headers("old_auth"),
        };
        let client_with_new_headers = HttpClientWithDefaultHeaders {
            inner: client_with_old_default_headers,
            headers: headers("new_auth"),
        };
        //when
        client_with_new_headers.request(Method::Get, "url".to_string(), None, None);
        //then
        assert_eq!(
            mock_client.next_request().unwrap().headers.unwrap().get(),
            Some(&Authorization("new_auth".to_string()))
        )
    }

    fn headers(auth_header: &str) -> Headers {
        let mut headers = Headers::new();
        headers.set(Authorization(auth_header.to_string()));
        headers
    }

    #[derive(Clone)]
    struct MockHttpClient {
        requests: Arc<Mutex<VecDeque<Request>>>,
    }

    #[derive(Debug, Clone)]
    struct Request {
        method: hyper::Method,
        url: String,
        body: Option<String>,
        headers: Option<Headers>,
    }

    impl MockHttpClient {
        fn new() -> MockHttpClient {
            MockHttpClient {
                requests: Arc::new(Mutex::new(VecDeque::new())),
            }
        }

        fn next_request(&self) -> Option<Request> {
            self.requests.lock().unwrap().pop_front()
        }
    }

    impl HttpClient for MockHttpClient {
        fn request(
            &self,
            method: hyper::Method,
            url: String,
            body: Option<String>,
            headers: Option<Headers>,
        ) -> Box<Future<Item = Response, Error = Error> + Send> {
            self.requests.lock().unwrap().push_back(Request {
                method,
                url,
                body,
                headers,
            });
            Box::new(future::ok(Response(String::new())))
        }
    }
}
