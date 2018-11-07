use failure::Error;
use futures::Future;

mod orders;
pub use self::orders::*;

mod stores;
pub use self::stores::*;

mod notifications;
pub use self::notifications::*;

mod users;
pub use self::users::*;

pub type ApiFuture<T> = Box<Future<Item = T, Error = Error> + Send>;
