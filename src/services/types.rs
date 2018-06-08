use failure::Error as FailureError;
use futures::future::Future;

/// Service layer Future
pub type ServiceFuture<SELF, T> = Box<Future<Item = (SELF, T), Error = (SELF, FailureError)>>;
