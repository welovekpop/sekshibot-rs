use lazy_static::lazy_static;
use rusqlite_migration::{Migrations, M};

lazy_static! {
    pub static ref MIGRATIONS: Migrations<'static> = Migrations::new(vec![
        M::up("
            CREATE TABLE emotes (
                name TEXT NOT NULL UNIQUE,
                url TEXT NOT NULL
            ) STRICT;
        "),
        M::up("
            CREATE TABLE skiplist (
                source_type TEXT NOT NULL,
                source_id TEXT NOT NULL,
                reason TEXT,
                PRIMARY KEY (source_type, source_id)
            ) STRICT;
        "),
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations() {
        MIGRATIONS.validate().unwrap();
    }
}
