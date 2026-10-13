//! 第五代仓颉：编码、字首／字身拆分与键位字根。

use std::collections::HashMap;
use std::sync::OnceLock;

/// Built by `scripts/build_cangjie.py`.
pub const DICT_TSV: &str = include_str!("../data/cangjie5.tsv");

/// 24 个字母对应的字根（X 是难字键，不对应字根）。
pub const ROOT_NAMES: [(char, char); 24] = [
    ('A', '日'),
    ('B', '月'),
    ('C', '金'),
    ('D', '木'),
    ('E', '水'),
    ('F', '火'),
    ('G', '土'),
    ('H', '竹'),
    ('I', '戈'),
    ('J', '十'),
    ('K', '大'),
    ('L', '中'),
    ('M', '一'),
    ('N', '弓'),
    ('O', '人'),
    ('P', '心'),
    ('Q', '手'),
    ('R', '口'),
    ('S', '尸'),
    ('T', '廿'),
    ('U', '山'),
    ('V', '女'),
    ('W', '田'),
    ('Y', '卜'),
];

pub fn root_name(key: char) -> Option<char> {
    ROOT_NAMES.iter().find(|(k, _)| *k == key).map(|(_, n)| *n)
}

/// 每个键上常见的辅助字形（不含字根本身），按出现次数排序。由 `scripts/build_cangjie.py` 生成。
pub fn key_shapes(key: char) -> &'static [char] {
    static TABLE: OnceLock<HashMap<char, Vec<char>>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            include_str!("../data/cangjie5_keys.tsv")
                .lines()
                .filter_map(|line| {
                    let mut cols = line.split('\t');
                    let key = cols.next()?.chars().next()?;
                    let _name = cols.next()?;
                    Some((key, cols.next().unwrap_or_default().chars().collect()))
                })
                .collect()
        })
        .get(&key)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// 整体字（含字根本身）：不分字首字身，取首、次、三、尾码。
    Whole,
    /// 字首 + 字身。
    Split,
    /// 结构数据和编码对不上，只能逐码讲解。
    Unknown,
}

impl Kind {
    fn parse(s: &str) -> Self {
        match s {
            "split" => Self::Split,
            "whole" => Self::Whole,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Whole => "整体字",
            Self::Split => "组合字",
            Self::Unknown => "组合字",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part {
    /// 部件字形；数据里没有对应的字时为空。
    pub glyph: Option<char>,
    /// 部件在这个字里取的码。
    pub code: String,
    /// 部件单独成字时的码（和这里取的码不同时才有，如「化」的匕取 P，单独打 UH）。
    pub standalone: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub ch: char,
    pub code: String,
    pub kind: Kind,
    /// 第一个是字首，其余是字身（字身再分时依次是次字首、次字身）。
    pub parts: Vec<Part>,
}

impl Entry {
    pub fn parse_line(line: &str) -> Option<Self> {
        let mut cols = line.split('\t');
        let ch = cols.next()?.chars().next()?;
        let code = cols.next()?.to_string();
        let kind = Kind::parse(cols.next()?);
        let parts = cols
            .next()?
            .split(' ')
            .filter_map(|tok| {
                let mut fields = tok.split(':');
                let glyph = fields.next()?.chars().next();
                let code = fields.next()?.to_string();
                let standalone = fields.next().map(str::to_string);
                Some(Part {
                    glyph,
                    code,
                    standalone,
                })
            })
            .collect();
        Some(Self {
            ch,
            code,
            kind,
            parts,
        })
    }

    /// 速成（简易）码：全码的首尾两码。
    pub fn quick_code(&self) -> String {
        let mut chars = self.code.chars();
        match (chars.next(), chars.last()) {
            (Some(a), Some(b)) => format!("{a}{b}"),
            _ => self.code.clone(),
        }
    }

    /// 每个部件的角色：字首、字身，字身再分时为次字首、次字身……
    pub fn part_roles(&self) -> Vec<&'static str> {
        match (self.kind, self.parts.len()) {
            (Kind::Split, 2) => vec!["字首", "字身"],
            (Kind::Split, n) => {
                let mut roles = vec!["字首"];
                for i in 1..n {
                    roles.push(match (i, i == n - 1) {
                        (1, _) => "次字首",
                        (_, true) => "次字身",
                        _ => "再字首",
                    });
                }
                roles
            }
            _ => vec![""; self.parts.len()],
        }
    }

    /// 键盘上要标出的部件字形：只占一码的部件。
    pub fn single_key_shapes(&self) -> Vec<(char, char)> {
        self.parts
            .iter()
            .filter_map(|p| {
                let mut keys = p.code.chars();
                match (p.glyph, keys.next(), keys.next()) {
                    (Some(g), Some(k), None) => Some((g, k)),
                    _ => None,
                }
            })
            .collect()
    }
}

#[derive(Default)]
pub struct Dictionary {
    entries: HashMap<char, Entry>,
}

impl Dictionary {
    pub fn load() -> Self {
        Self {
            entries: DICT_TSV
                .lines()
                .filter_map(Entry::parse_line)
                .map(|e| (e.ch, e))
                .collect(),
        }
    }

    pub fn get(&self, ch: char) -> Option<&Entry> {
        self.entries.get(&ch)
    }

    pub fn contains(&self, ch: char) -> bool {
        self.entries.contains_key(&ch)
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_splits() {
        let d = Dictionary::load();
        assert!(d.len() > 20000);
        let e = d.get('明').unwrap();
        assert_eq!(e.code, "AB");
        assert_eq!(e.kind, Kind::Split);
        assert_eq!(e.part_roles(), ["字首", "字身"]);
        assert_eq!(e.single_key_shapes(), [('日', 'A'), ('月', 'B')]);
        assert_eq!(d.get('键').unwrap().quick_code(), "OQ");
        assert_eq!(d.get('东').unwrap().kind, Kind::Whole);
        // 部件单独成字时的码不同。
        let wo = d.get('我').unwrap();
        assert_eq!(wo.code, "HQI");
        assert_eq!(wo.parts[0].standalone.as_deref(), Some("Q"));
        for ch in "我们的是学习仓颉照键盘".chars() {
            assert!(d.contains(ch), "{ch}");
        }
    }

    #[test]
    fn key_table() {
        assert_eq!(root_name('A'), Some('日'));
        assert_eq!(root_name('X'), None);
        assert!(key_shapes('E').contains(&'氵'));
        assert!(key_shapes('Q').contains(&'扌'));
    }
}
