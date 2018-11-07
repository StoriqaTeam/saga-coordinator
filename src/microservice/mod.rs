use failure::Error;
use futures::Future;

mod orders;
pub use self::orders::*;

mod stores;
pub use self::stores::*;

pub type ApiFuture<T> = Box<Future<Item = T, Error = Error> + Send>;
