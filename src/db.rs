use crate::errors::GeyserError;
use solana_program::pubkey::Pubkey;
use sqlite::Connection;

const DB_KEY_FILTERS: &str = "filters";

pub struct DB {
    conn: Connection,
}

impl DB {
    pub fn new(path: String) -> Result<Self, GeyserError<'static>> {
        let connection = sqlite::open(path).map_err(GeyserError::SqliteError)?;

        let query = "
         CREATE TABLE IF NOT EXISTS settings(
            key varchar(32) NOT NULL,
            value text NOT NULL,
            PRIMARY KEY(key)
         );";

        connection
            .execute(query)
            .map_err(GeyserError::SqliteError)?;

        Ok(Self { conn: connection })
    }

    pub fn set_filters(&self, v: Vec<Pubkey>) -> Result<(), GeyserError> {
        let query = "
        INSERT INTO settings (key, value)
            VALUES (:key, :value)
            ON CONFLICT(key)
            DO UPDATE SET value=excluded.value;";

        let mut statement = self.conn.prepare(query).map_err(GeyserError::SqliteError)?;

        let filters =
            serde_json::to_string(&v).map_err(|_| GeyserError::CustomError("cannot serialize"))?;

        statement
            .bind_iter([(":key", DB_KEY_FILTERS), (":value", filters.as_str())])
            .map_err(GeyserError::SqliteError)?;
        statement.next().map_err(GeyserError::SqliteError)?;

        Ok(())
    }

    pub fn get_filters(&self) -> Result<Vec<Pubkey>, GeyserError> {
        let query = "SELECT value FROM settings WHERE key = ?";
        let mut statement = self.conn.prepare(query).map_err(GeyserError::SqliteError)?;
        statement
            .bind((1, DB_KEY_FILTERS))
            .map_err(GeyserError::SqliteError)?;

        statement.next().map_err(GeyserError::SqliteError)?;
        let val = statement
            .read::<String, _>("value")
            .map_err(GeyserError::SqliteError)?;

        let data: Vec<Pubkey> = serde_json::from_str(&val)
            .map_err(|_| GeyserError::CustomError("cannot deserialize"))?;

        return Ok(data);
    }
}
