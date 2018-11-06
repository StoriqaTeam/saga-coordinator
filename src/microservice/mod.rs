use failure::Error;
use futures::Future;

mod orders;
pub use self::orders::*;

pub type ApiFuture<T> = Box<Future<Item = T, Error = Error> + Send>;
