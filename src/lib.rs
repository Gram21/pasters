#[macro_use]
extern crate diesel_codegen;
#[macro_use]
extern crate diesel;
extern crate dotenv;
extern crate r2d2;
extern crate r2d2_diesel;
extern crate rand;
extern crate rocket;

pub mod schema;
pub mod models;

use diesel::pg::PgConnection;
use dotenv::dotenv;
use r2d2::{Pool, Config};
use r2d2_diesel::ConnectionManager;
use std::env;

pub fn create_db_pool() -> Pool<ConnectionManager<PgConnection>> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let config = Config::default();
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Pool::new(config, manager).expect("Failed to create pool.")
}
