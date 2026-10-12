//! 应用共享的数据：数据库连接和内存字典。

use anyhow::Result;

use crate::db::Db;
use crate::dict::Dictionary;

pub struct Store {
    pub db: Db,
    pub dict: Dictionary,
}

impl Store {
    pub fn open() -> Result<Self> {
        let db = Db::open(&Db::default_path())?;
        let dict = db.load_dictionary()?;
        Ok(Self { db, dict })
    }
}
