//! Unit 2.9 proof artifacts: the `artifacts` table (`artifacts.rs` holds its store
//! functions). Migrations 0900 to 0949, files `migrations/09xx_<name>.sql`.

use super::migrate::Migration;

pub const MIGRATIONS: &[Migration] = &[Migration {
    id: 900,
    name: "0900_artifacts",
    sql: include_str!("migrations/0900_artifacts.sql"),
}];
