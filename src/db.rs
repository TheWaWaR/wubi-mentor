//! SQLite 存储：字典、练习文本、进度、逐字熟练度与练习记录。

use std::cell::Cell;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use rusqlite::{Connection, OptionalExtension, params};

use crate::dict::{DICT_TSV, DICT_VERSION, Dictionary, Entry};
use crate::scheme::Scheme;

/// 逐字统计和练习记录按当前方案分开存。
pub struct Db {
    conn: Connection,
    scheme: Cell<Scheme>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub id: i64,
    pub title: String,
    pub content: String,
    /// 下一个要打的字在 `content.chars()` 中的下标。
    pub position: usize,
    pub updated_at: i64,
    pub last_practiced_at: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CharStat {
    pub ch: char,
    /// 练过多少次（每次打对算一次）。
    pub seen: i64,
    /// 打错的次数（一次练习中可能多次）。
    pub errors: i64,
    /// 有错误的练习次数。
    pub missed: i64,
    pub total_ms: i64,
    /// 连续一次打对的次数，出错清零。
    pub streak: i64,
    pub last_seen: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mastery {
    New,
    Weak,
    Learning,
    Mastered,
}

impl Mastery {
    pub fn label(self) -> &'static str {
        match self {
            Self::New => "未练",
            Self::Weak => "薄弱",
            Self::Learning => "练习中",
            Self::Mastered => "已掌握",
        }
    }
}

impl CharStat {
    pub fn avg_ms(&self) -> Option<i64> {
        (self.seen > 0).then(|| self.total_ms / self.seen)
    }

    pub fn miss_rate(&self) -> f64 {
        if self.seen == 0 {
            0.0
        } else {
            self.missed as f64 / self.seen as f64
        }
    }

    pub fn mastery(&self) -> Mastery {
        if self.seen == 0 {
            Mastery::New
        } else if self.streak >= 3 && self.avg_ms().unwrap_or(i64::MAX) < 4000 {
            Mastery::Mastered
        } else if self.streak == 0 || self.miss_rate() > 0.4 {
            Mastery::Weak
        } else {
            Mastery::Learning
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub id: i64,
    pub text_id: Option<i64>,
    pub text_title: Option<String>,
    pub started_at: i64,
    pub ended_at: i64,
    pub active_ms: i64,
    pub chars: i64,
    pub missed: i64,
}

impl Session {
    /// 每分钟打对的字数。
    pub fn speed(&self) -> f64 {
        if self.active_ms < 1000 {
            0.0
        } else {
            self.chars as f64 * 60_000.0 / self.active_ms as f64
        }
    }

    /// 一次打对的比例。
    pub fn accuracy(&self) -> f64 {
        if self.chars == 0 {
            1.0
        } else {
            1.0 - self.missed as f64 / self.chars as f64
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HintMode {
    /// 一直显示当前字的拆分。
    Always,
    /// 打错或停顿时才显示。
    OnMiss,
    /// 只在点击或查询时显示。
    Never,
}

impl HintMode {
    pub const ALL: [HintMode; 3] = [Self::Always, Self::OnMiss, Self::Never];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::OnMiss => "on_miss",
            Self::Never => "never",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "on_miss" => Self::OnMiss,
            "never" => Self::Never,
            _ => Self::Always,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Always => "始终提示",
            Self::OnMiss => "出错再提示",
            Self::Never => "不提示",
        }
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS texts (
    id                INTEGER PRIMARY KEY,
    title             TEXT NOT NULL,
    content           TEXT NOT NULL,
    position          INTEGER NOT NULL DEFAULT 0,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    last_practiced_at INTEGER
);
CREATE TABLE IF NOT EXISTS char_stats (
    scheme    TEXT NOT NULL DEFAULT 'wubi86',
    ch        TEXT NOT NULL,
    seen      INTEGER NOT NULL DEFAULT 0,
    errors    INTEGER NOT NULL DEFAULT 0,
    missed    INTEGER NOT NULL DEFAULT 0,
    total_ms  INTEGER NOT NULL DEFAULT 0,
    streak    INTEGER NOT NULL DEFAULT 0,
    last_seen INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (scheme, ch)
);
CREATE TABLE IF NOT EXISTS sessions (
    id         INTEGER PRIMARY KEY,
    text_id    INTEGER REFERENCES texts(id) ON DELETE SET NULL,
    started_at INTEGER NOT NULL,
    ended_at   INTEGER NOT NULL,
    active_ms  INTEGER NOT NULL,
    chars      INTEGER NOT NULL,
    missed     INTEGER NOT NULL,
    scheme     TEXT NOT NULL DEFAULT 'wubi86'
);
";

/// 只有五笔时建的库：`char_stats` 以字为主键、`sessions` 没有方案列，旧数据都算五笔。
const MIGRATE_SCHEME: &str = "
ALTER TABLE char_stats RENAME TO char_stats_old;
CREATE TABLE char_stats (
    scheme    TEXT NOT NULL DEFAULT 'wubi86',
    ch        TEXT NOT NULL,
    seen      INTEGER NOT NULL DEFAULT 0,
    errors    INTEGER NOT NULL DEFAULT 0,
    missed    INTEGER NOT NULL DEFAULT 0,
    total_ms  INTEGER NOT NULL DEFAULT 0,
    streak    INTEGER NOT NULL DEFAULT 0,
    last_seen INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (scheme, ch)
);
INSERT INTO char_stats (scheme, ch, seen, errors, missed, total_ms, streak, last_seen)
    SELECT 'wubi86', ch, seen, errors, missed, total_ms, streak, last_seen FROM char_stats_old;
DROP TABLE char_stats_old;
ALTER TABLE sessions ADD COLUMN scheme TEXT NOT NULL DEFAULT 'wubi86';
";

/// 字典表跟着字典版本整表重建，所以单独建。
const CHARS_SCHEMA: &str = "
DROP TABLE IF EXISTS chars;
CREATE TABLE chars (
    ch          TEXT PRIMARY KEY,
    full_code   TEXT NOT NULL,
    short_codes TEXT NOT NULL,
    kind        TEXT NOT NULL,
    roots       TEXT NOT NULL,
    ident       TEXT NOT NULL,
    glyphs      TEXT NOT NULL
);
";

const SAMPLE_TEXTS: [(&str, &str); 3] = [
    (
        "一级简码",
        "一地在要工上是中国同和的有人我主产不为这民了发以经",
    ),
    (
        "常用字入门",
        "学习五笔的第一步是认识字根。每个汉字都可以拆成几个字根，再按照书写的顺序取码。\
         刚开始的时候速度会很慢，这是正常的。不要急着追求速度，先把每个字拆对，\
         手指自然会记住键位。每天练习一会儿，坚持一个月，你就会发现自己打字又快又准。",
    ),
    (
        "桃花源记（节选）",
        "晋太元中，武陵人捕鱼为业。缘溪行，忘路之远近。忽逢桃花林，夹岸数百步，中无杂树，\
         芳草鲜美，落英缤纷。渔人甚异之，复前行，欲穷其林。林尽水源，便得一山，山有小口，\
         仿佛若有光。便舍船，从口入。初极狭，才通人。复行数十步，豁然开朗。土地平旷，\
         屋舍俨然，有良田美池桑竹之属。阡陌交通，鸡犬相闻。",
    ),
];

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        }
        let conn = Connection::open(path).with_context(|| format!("open {}", path.display()))?;
        Self::init(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        conn.execute_batch(SCHEMA)?;
        let has_scheme: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('sessions') WHERE name = 'scheme'",
            [],
            |r| r.get(0),
        )?;
        if !has_scheme {
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(MIGRATE_SCHEME)?;
            tx.commit()?;
        }
        let db = Self {
            conn,
            scheme: Cell::new(Scheme::Wubi86),
        };
        if let Some(s) = db.meta("scheme")? {
            db.scheme.set(Scheme::parse(&s));
        }
        db.seed_dictionary()?;
        db.seed_texts()?;
        Ok(db)
    }

    /// 默认在 `~/Library/Application Support/zigen/zigen.db`；可用 `ZIGEN_DB` 覆盖。
    /// 旧版（wubi-mentor）的数据库会在第一次用默认路径时搬过来。
    pub fn default_path() -> Result<PathBuf> {
        if let Some(path) = std::env::var_os("ZIGEN_DB") {
            return Ok(path.into());
        }
        let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        let path = base.join("zigen").join("zigen.db");
        migrate_legacy(&base.join("wubi-mentor").join("wubi.db"), &path)?;
        Ok(path)
    }

    fn meta(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }

    fn seed_dictionary(&self) -> Result<()> {
        if self.meta("dict_version")?.as_deref() == Some(DICT_VERSION) {
            return Ok(());
        }
        let tx = self.conn.unchecked_transaction()?;
        tx.execute_batch(CHARS_SCHEMA)?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO chars (ch, full_code, short_codes, kind, roots, ident, glyphs)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for e in Dictionary::parse_tsv(DICT_TSV) {
                stmt.execute(params![
                    e.ch.to_string(),
                    e.full_code,
                    e.short_codes.join(","),
                    e.kind.as_str(),
                    e.roots_column(),
                    e.ident.map(String::from).unwrap_or_default(),
                    e.code_glyphs.iter().collect::<String>(),
                ])?;
            }
        }
        tx.execute(
            "INSERT INTO meta (key, value) VALUES ('dict_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [DICT_VERSION],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn seed_texts(&self) -> Result<()> {
        if self.meta("seeded_texts")?.is_some() {
            return Ok(());
        }
        for (title, content) in SAMPLE_TEXTS {
            self.insert_text(title, content)?;
        }
        self.set_meta("seeded_texts", "1")
    }

    pub fn load_dictionary(&self) -> Result<Dictionary> {
        let mut stmt = self
            .conn
            .prepare("SELECT ch, full_code, short_codes, kind, roots, ident, glyphs FROM chars")?;
        let rows = stmt.query_map([], |r| {
            let cols: [String; 7] = [
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ];
            Ok(cols.join("\t"))
        })?;
        let mut entries = Vec::new();
        for line in rows {
            if let Some(e) = Entry::parse_line(&line?) {
                entries.push(e);
            }
        }
        Ok(Dictionary::from_entries(entries))
    }

    // ---- texts ----

    pub fn texts(&self) -> Result<Vec<Text>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, content, position, updated_at, last_practiced_at FROM texts
             ORDER BY COALESCE(last_practiced_at, updated_at) DESC, id DESC",
        )?;
        let rows = stmt.query_map([], row_to_text)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn text(&self, id: i64) -> Result<Option<Text>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, title, content, position, updated_at, last_practiced_at
                 FROM texts WHERE id = ?1",
                [id],
                row_to_text,
            )
            .optional()?)
    }

    pub fn insert_text(&self, title: &str, content: &str) -> Result<i64> {
        let now = now_ms();
        self.conn.execute(
            "INSERT INTO texts (title, content, position, created_at, updated_at)
             VALUES (?1, ?2, 0, ?3, ?3)",
            params![title, content, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// 修改内容时，若原进度之前的内容没变则保留进度，否则从头开始。
    pub fn update_text(&self, id: i64, title: &str, content: &str) -> Result<()> {
        let Some(old) = self.text(id)? else {
            return Ok(());
        };
        let kept: String = old.content.chars().take(old.position).collect();
        let position = if content.starts_with(&kept) {
            old.position
        } else {
            0
        };
        self.conn.execute(
            "UPDATE texts SET title = ?2, content = ?3, position = ?4, updated_at = ?5
             WHERE id = ?1",
            params![id, title, content, position as i64, now_ms()],
        )?;
        Ok(())
    }

    pub fn delete_text(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM texts WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn set_position(&self, id: i64, position: usize) -> Result<()> {
        self.conn.execute(
            "UPDATE texts SET position = ?2, last_practiced_at = ?3 WHERE id = ?1",
            params![id, position as i64, now_ms()],
        )?;
        Ok(())
    }

    // ---- settings ----

    pub fn current_text_id(&self) -> Result<Option<i64>> {
        Ok(self.meta("current_text")?.and_then(|v| v.parse().ok()))
    }

    pub fn set_current_text_id(&self, id: i64) -> Result<()> {
        self.set_meta("current_text", &id.to_string())
    }

    pub fn scheme(&self) -> Scheme {
        self.scheme.get()
    }

    pub fn set_scheme(&self, scheme: Scheme) -> Result<()> {
        self.scheme.set(scheme);
        self.set_meta("scheme", scheme.as_str())
    }

    pub fn hint_mode(&self) -> Result<HintMode> {
        Ok(self
            .meta("hint_mode")?
            .map(|v| HintMode::parse(&v))
            .unwrap_or(HintMode::Always))
    }

    pub fn set_hint_mode(&self, mode: HintMode) -> Result<()> {
        self.set_meta("hint_mode", mode.as_str())
    }

    // ---- per-char stats ----

    /// 记录一个字打对了：`missed` 表示这次之前打错过 `errors` 次。
    pub fn record_char(&self, ch: char, ms: i64, errors: u32) -> Result<()> {
        let missed = i64::from(errors > 0);
        self.conn.execute(
            "INSERT INTO char_stats (scheme, ch, seen, errors, missed, total_ms, streak, last_seen)
             VALUES (?7, ?1, 1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(scheme, ch) DO UPDATE SET
                seen = seen + 1,
                errors = errors + excluded.errors,
                missed = missed + excluded.missed,
                total_ms = total_ms + excluded.total_ms,
                streak = CASE WHEN excluded.missed > 0 THEN 0 ELSE streak + 1 END,
                last_seen = excluded.last_seen",
            params![
                ch.to_string(),
                errors,
                missed,
                ms,
                1 - missed,
                now_ms(),
                self.scheme().as_str()
            ],
        )?;
        Ok(())
    }

    pub fn char_stat(&self, ch: char) -> Result<CharStat> {
        Ok(self
            .conn
            .query_row(
                "SELECT ch, seen, errors, missed, total_ms, streak, last_seen
                 FROM char_stats WHERE scheme = ?1 AND ch = ?2",
                [self.scheme().as_str(), &ch.to_string()],
                row_to_stat,
            )
            .optional()?
            .unwrap_or(CharStat {
                ch,
                ..Default::default()
            }))
    }

    pub fn char_stats(&self) -> Result<Vec<CharStat>> {
        let mut stmt = self.conn.prepare(
            "SELECT ch, seen, errors, missed, total_ms, streak, last_seen FROM char_stats
             WHERE scheme = ?1",
        )?;
        let rows = stmt.query_map([self.scheme().as_str()], row_to_stat)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// 最需要重练的字：出错多、最近一次出错、或打得很慢。
    pub fn weak_chars(&self, limit: usize) -> Result<Vec<CharStat>> {
        let mut stats: Vec<CharStat> = self
            .char_stats()?
            .into_iter()
            .filter(|s| s.mastery() == Mastery::Weak)
            .collect();
        stats.sort_by(|a, b| {
            b.miss_rate()
                .total_cmp(&a.miss_rate())
                .then(b.errors.cmp(&a.errors))
                .then(b.last_seen.cmp(&a.last_seen))
        });
        stats.truncate(limit);
        Ok(stats)
    }

    // ---- sessions ----

    pub fn insert_session(
        &self,
        text_id: Option<i64>,
        started_at: i64,
        ended_at: i64,
        active_ms: i64,
        chars: i64,
        missed: i64,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO sessions (text_id, started_at, ended_at, active_ms, chars, missed, scheme)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                text_id,
                started_at,
                ended_at,
                active_ms,
                chars,
                missed,
                self.scheme().as_str()
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn sessions(&self, limit: usize) -> Result<Vec<Session>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.text_id, t.title, s.started_at, s.ended_at, s.active_ms, s.chars, s.missed
             FROM sessions s LEFT JOIN texts t ON t.id = s.text_id
             WHERE s.scheme = ?2
             ORDER BY s.started_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64, self.scheme().as_str()], |r| {
            Ok(Session {
                id: r.get(0)?,
                text_id: r.get(1)?,
                text_title: r.get(2)?,
                started_at: r.get(3)?,
                ended_at: r.get(4)?,
                active_ms: r.get(5)?,
                chars: r.get(6)?,
                missed: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

fn row_to_text(r: &rusqlite::Row) -> rusqlite::Result<Text> {
    Ok(Text {
        id: r.get(0)?,
        title: r.get(1)?,
        content: r.get(2)?,
        position: r.get::<_, i64>(3)?.max(0) as usize,
        updated_at: r.get(4)?,
        last_practiced_at: r.get(5)?,
    })
}

fn row_to_stat(r: &rusqlite::Row) -> rusqlite::Result<CharStat> {
    Ok(CharStat {
        ch: r.get::<_, String>(0)?.chars().next().unwrap_or(' '),
        seen: r.get(1)?,
        errors: r.get(2)?,
        missed: r.get(3)?,
        total_ms: r.get(4)?,
        streak: r.get(5)?,
        last_seen: r.get(6)?,
    })
}

/// 把旧数据库（连同 WAL 的 `-wal`/`-shm` 文件）改名搬到新路径；新路径已存在或旧库不存在时什么都不做。
fn migrate_legacy(old: &Path, new: &Path) -> Result<()> {
    if new.exists() || !old.exists() {
        return Ok(());
    }
    if let Some(dir) = new.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    }
    // 先搬 -wal/-shm，最后搬主库：中途失败时新路径上没有主库，下次启动会重试。
    for suffix in ["-wal", "-shm", ""] {
        let from = PathBuf::from(format!("{}{suffix}", old.display()));
        let to = PathBuf::from(format!("{}{suffix}", new.display()));
        if from.exists() {
            std::fs::rename(&from, &to)
                .with_context(|| format!("move {} to {}", from.display(), to.display()))?;
        }
    }
    if let Some(dir) = old.parent() {
        let _ = std::fs::remove_dir(dir); // 只删空目录
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_dictionary_and_texts() {
        let db = Db::open_in_memory().unwrap();
        let dict = db.load_dictionary().unwrap();
        assert!(dict.len() > 9000);
        assert_eq!(dict.get('的').unwrap().full_code, "RQYY");
        assert_eq!(db.texts().unwrap().len(), SAMPLE_TEXTS.len());
    }

    #[test]
    fn edit_keeps_progress_when_prefix_unchanged() {
        let db = Db::open_in_memory().unwrap();
        let id = db.insert_text("t", "我们学习五笔").unwrap();
        db.set_position(id, 2).unwrap();
        db.update_text(id, "t", "我们学习五笔打字").unwrap();
        assert_eq!(db.text(id).unwrap().unwrap().position, 2);
        db.update_text(id, "t", "你们学习").unwrap();
        assert_eq!(db.text(id).unwrap().unwrap().position, 0);
    }

    #[test]
    fn char_stats_track_streak_and_mastery() {
        let db = Db::open_in_memory().unwrap();
        db.record_char('我', 2000, 1).unwrap();
        let s = db.char_stat('我').unwrap();
        assert_eq!((s.seen, s.errors, s.missed, s.streak), (1, 1, 1, 0));
        assert_eq!(s.mastery(), Mastery::Weak);
        for _ in 0..3 {
            db.record_char('我', 1000, 0).unwrap();
        }
        let s = db.char_stat('我').unwrap();
        assert_eq!((s.seen, s.streak), (4, 3));
        assert_eq!(s.mastery(), Mastery::Mastered);
        assert!(db.weak_chars(10).unwrap().is_empty());
    }

    #[test]
    fn sessions_join_text_title() {
        let db = Db::open_in_memory().unwrap();
        let id = db.insert_text("标题", "内容").unwrap();
        db.insert_session(Some(id), 0, 60_000, 60_000, 30, 3)
            .unwrap();
        let s = &db.sessions(10).unwrap()[0];
        assert_eq!(s.text_title.as_deref(), Some("标题"));
        assert_eq!(s.speed(), 30.0);
        assert!((s.accuracy() - 0.9).abs() < 1e-9);
    }

    #[test]
    fn stats_are_per_scheme() {
        let db = Db::open_in_memory().unwrap();
        db.record_char('我', 1000, 0).unwrap();
        db.insert_session(None, 0, 1000, 1000, 1, 0).unwrap();
        db.set_scheme(Scheme::Cangjie5).unwrap();
        assert_eq!(db.char_stat('我').unwrap().seen, 0);
        assert!(db.sessions(10).unwrap().is_empty());
        db.record_char('我', 1000, 1).unwrap();
        assert_eq!(db.char_stats().unwrap().len(), 1);
        db.set_scheme(Scheme::Wubi86).unwrap();
        assert_eq!(db.char_stat('我').unwrap().errors, 0);
        assert_eq!(db.sessions(10).unwrap().len(), 1);
    }

    #[test]
    fn migrates_single_scheme_tables() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE char_stats (ch TEXT PRIMARY KEY, seen INTEGER NOT NULL DEFAULT 0,
                errors INTEGER NOT NULL DEFAULT 0, missed INTEGER NOT NULL DEFAULT 0,
                total_ms INTEGER NOT NULL DEFAULT 0, streak INTEGER NOT NULL DEFAULT 0,
                last_seen INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE sessions (id INTEGER PRIMARY KEY, text_id INTEGER, started_at INTEGER NOT NULL,
                ended_at INTEGER NOT NULL, active_ms INTEGER NOT NULL, chars INTEGER NOT NULL,
                missed INTEGER NOT NULL);
             INSERT INTO char_stats (ch, seen) VALUES ('我', 5);
             INSERT INTO sessions (started_at, ended_at, active_ms, chars, missed) VALUES (0, 1, 1, 1, 0);",
        )
        .unwrap();
        let db = Db::init(conn).unwrap();
        assert_eq!(db.scheme(), Scheme::Wubi86);
        assert_eq!(db.char_stat('我').unwrap().seen, 5);
        assert_eq!(db.sessions(10).unwrap().len(), 1);
        db.set_scheme(Scheme::Cangjie5).unwrap();
        assert_eq!(db.char_stat('我').unwrap().seen, 0);
    }

    #[test]
    fn migrates_legacy_database() {
        let root = std::env::temp_dir().join(format!("zigen-migrate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let old = root.join("wubi-mentor").join("wubi.db");
        let new = root.join("zigen").join("zigen.db");
        {
            let db = Db::open(&old).unwrap();
            db.insert_text("旧文本", "内容").unwrap();
        }
        migrate_legacy(&old, &new).unwrap();
        assert!(!old.parent().unwrap().exists());
        let db = Db::open(&new).unwrap();
        assert!(db.texts().unwrap().iter().any(|t| t.title == "旧文本"));
        // 新库已存在时不再动旧路径。
        migrate_legacy(&old, &new).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
    }
}
