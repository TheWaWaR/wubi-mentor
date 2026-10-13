//! 应用共享的数据：数据库连接和内存字典。

use anyhow::Result;

use crate::cangjie;
use crate::db::Db;
use crate::dict::Dictionary;
use crate::scheme::Scheme;

pub struct Store {
    pub db: Db,
    /// 86 版五笔字典。
    pub dict: Dictionary,
    pub cangjie: cangjie::Dictionary,
}

/// 当前方案下一个字的拆分。
#[derive(Clone, Copy)]
pub enum Lookup<'a> {
    Wubi(&'a crate::dict::Entry),
    Cangjie(&'a cangjie::Entry),
}

impl Lookup<'_> {
    pub fn ch(&self) -> char {
        match self {
            Self::Wubi(e) => e.ch,
            Self::Cangjie(e) => e.ch,
        }
    }

    /// 全码，用于键盘高亮。
    pub fn code(&self) -> &str {
        match self {
            Self::Wubi(e) => &e.full_code,
            Self::Cangjie(e) => &e.code,
        }
    }
}

impl Store {
    pub fn open() -> Result<Self> {
        let db = Db::open(&Db::default_path()?)?;
        let dict = db.load_dictionary()?;
        Ok(Self {
            db,
            dict,
            cangjie: cangjie::Dictionary::load(),
        })
    }

    pub fn scheme(&self) -> Scheme {
        self.db.scheme()
    }

    pub fn lookup(&self, ch: char) -> Option<Lookup<'_>> {
        match self.scheme() {
            Scheme::Wubi86 => self.dict.get(ch).map(Lookup::Wubi),
            Scheme::Cangjie5 => self.cangjie.get(ch).map(Lookup::Cangjie),
        }
    }

    /// 当前方案的字典里有没有这个字（决定练习时要不要打）。
    pub fn contains(&self, ch: char) -> bool {
        match self.scheme() {
            Scheme::Wubi86 => self.dict.contains(ch),
            Scheme::Cangjie5 => self.cangjie.contains(ch),
        }
    }
}
