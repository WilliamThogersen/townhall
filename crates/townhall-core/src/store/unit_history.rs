use super::migrate::Migration;

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        id: 1000,
        name: "1000_history_index",
        sql: include_str!("migrations/1000_history_index.sql"),
    },
    Migration {
        id: 1001,
        name: "1001_default_history_tools",
        sql: include_str!("migrations/1001_default_history_tools.sql"),
    },
];
