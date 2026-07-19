mod error;
mod pool;

pub mod console;
pub mod dao;
pub mod models;

pub use error::DbError;
pub use pool::{connect, DbPool};
