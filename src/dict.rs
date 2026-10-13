//! 86 版五笔拆字数据：字根、全码、简码与识别码。

use std::collections::HashMap;
use std::sync::OnceLock;

/// Built by `scripts/build_dict.py`; seeded into SQLite on first run.
pub const DICT_TSV: &str = include_str!("../data/wubi86.tsv");
/// Bump when `data/wubi86.tsv` changes so existing databases reseed.
pub const DICT_VERSION: &str = "2";

/// 键位 → 键名字根，按 QWERTY 键盘顺序可用 [`KEY_ROWS`] 访问。
pub const KEY_NAMES: [(char, char); 25] = [
    ('G', '王'),
    ('F', '土'),
    ('D', '大'),
    ('S', '木'),
    ('A', '工'),
    ('H', '目'),
    ('J', '日'),
    ('K', '口'),
    ('L', '田'),
    ('M', '山'),
    ('T', '禾'),
    ('R', '白'),
    ('E', '月'),
    ('W', '人'),
    ('Q', '金'),
    ('Y', '言'),
    ('U', '立'),
    ('I', '水'),
    ('O', '火'),
    ('P', '之'),
    ('N', '已'),
    ('B', '子'),
    ('V', '女'),
    ('C', '又'),
    ('X', '纟'),
];

pub const KEY_ROWS: [&str; 3] = ["QWERTYUIOP", "ASDFGHJKL", "ZXCVBNM"];

struct KeyRoots {
    all: Vec<char>,
    frequent: Vec<char>,
}

fn key_roots_table() -> &'static HashMap<char, KeyRoots> {
    static TABLE: OnceLock<HashMap<char, KeyRoots>> = OnceLock::new();
    TABLE.get_or_init(|| {
        include_str!("../data/key_roots.tsv")
            .lines()
            .filter_map(|line| {
                let mut cols = line.split('\t');
                let key = cols.next()?.chars().next()?;
                let all = cols.next()?.chars().collect();
                let frequent = cols.next().unwrap_or_default().chars().collect();
                Some((key, KeyRoots { all, frequent }))
            })
            .collect()
    })
}

/// 每个键上的全部字根在字根字体里的字形（86 版，不含繁体），按字根表顺序，第一个是键名字根。
/// 由 `scripts/build_dict.py` 生成。
pub fn key_roots(key: char) -> &'static [char] {
    key_roots_table()
        .get(&key)
        .map(|r| r.all.as_slice())
        .unwrap_or_default()
}

/// 每个键上最常用的字根（不含键名字根），按字根表顺序。
pub fn key_roots_frequent(key: char) -> &'static [char] {
    key_roots_table()
        .get(&key)
        .map(|r| r.frequent.as_slice())
        .unwrap_or_default()
}

/// 字根字体（「黑体字根」）的族名；它把字根和变形部件放在私用区。
pub const ROOT_FONT: &str = "Wubi Roots";
pub const ROOT_FONT_DATA: &[u8] = include_bytes!("../assets/fonts/wubi-roots.ttf");

/// 系统字体缺字形的字根，换成形近的字或文字描述来显示。
pub fn display_glyph(glyph: &str) -> &str {
    match glyph {
        "𡭔" => "小",
        "㇟" => "乚",
        "𠘧" => "几",
        "𰀪" => "勿内",
        "𠄐" => "了",
        "𣥂" => "步底",
        "𡆧" => "罒",
        "𫶧" => "儿",
        "𠃜" => "尸头",
        "𧰨" => "象底",
        "𰀁" => "十",
        "𱼀" => "夕",
        "𠚤" => "乂",
        "𠧒" => "止",
        "㇒" => "丿",
        "𫩏" => "象中",
        other => other,
    }
}

/// 单笔画（一丨丿丶乙）：报户口 + 再打一次 + 补两个 L。
pub fn is_single_stroke(ch: char) -> bool {
    "一丨丿丶乙".contains(ch)
}

pub fn key_name(key: char) -> Option<char> {
    KEY_NAMES.iter().find(|(k, _)| *k == key).map(|(_, n)| *n)
}

/// 一个字在 86 版五笔中属于哪一类。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharKind {
    /// 键名字根：连击四次本键。
    KeyName,
    /// 成字字根：报户口 + 首、次、末笔。
    Radical,
    /// 合体字：按书写顺序取字根。
    Compound,
}

impl CharKind {
    pub fn parse(s: &str) -> Self {
        match s {
            "key" => Self::KeyName,
            "radical" => Self::Radical,
            _ => Self::Compound,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::KeyName => "key",
            Self::Radical => "radical",
            Self::Compound => "char",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::KeyName => "键名字根",
            Self::Radical => "成字字根",
            Self::Compound => "合体字",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Root {
    /// 字根的字形；伪字根（如“牛前”）是描述文字。
    pub glyph: String,
    pub key: char,
    /// 字根字体里对应的字形（私用区字符），能更准确地显示变形部件。
    pub font_glyph: Option<char>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub ch: char,
    pub full_code: String,
    pub short_codes: Vec<String>,
    pub kind: CharKind,
    pub roots: Vec<Root>,
    /// 末笔识别码所在的键（只有字根不足四码时才有）。
    pub ident: Option<char>,
    /// 拆分与编码是否互相印证；否则拆分只供参考。
    pub verified: bool,
    /// 每一码在字根字体里的字形：字根、识别码或笔画；可能为空。
    pub code_glyphs: Vec<char>,
}

/// 编码中每一码的来源，用于逐码讲解。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodePart {
    Root {
        index: usize,
        key: char,
    },
    Ident {
        key: char,
        stroke: &'static str,
        shape: &'static str,
    },
    Stroke {
        key: char,
        stroke: &'static str,
    },
    KeyRepeat {
        key: char,
    },
}

impl CodePart {
    pub fn key(&self) -> char {
        match self {
            Self::Root { key, .. }
            | Self::Ident { key, .. }
            | Self::Stroke { key, .. }
            | Self::KeyRepeat { key } => *key,
        }
    }
}

impl Entry {
    pub fn parse_line(line: &str) -> Option<Self> {
        let mut cols = line.split('\t');
        let ch = cols.next()?.chars().next()?;
        let full_code = cols.next()?.to_string();
        let short_codes = cols
            .next()?
            .split(',')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        let kind = CharKind::parse(cols.next()?);
        let (roots, verified) = parse_roots(cols.next()?);
        let ident = cols.next().and_then(|s| s.chars().next());
        let code_glyphs = cols.next().map(|s| s.chars().collect()).unwrap_or_default();
        Some(Self {
            ch,
            full_code,
            short_codes,
            kind,
            roots,
            ident,
            verified,
            code_glyphs,
        })
    }

    pub fn roots_column(&self) -> String {
        let body = self
            .roots
            .iter()
            .map(|r| match r.font_glyph {
                Some(g) => format!("{}:{}:{g}", r.glyph, r.key),
                None => format!("{}:{}", r.glyph, r.key),
            })
            .collect::<Vec<_>>()
            .join(" ");
        if self.verified {
            body
        } else {
            format!("!{body}")
        }
    }

    /// 全码每一码的由来。
    pub fn code_parts(&self) -> Vec<CodePart> {
        let keys: Vec<char> = self.full_code.chars().collect();
        match self.kind {
            CharKind::KeyName => keys
                .iter()
                .enumerate()
                .map(|(i, &key)| {
                    if i == 0 {
                        CodePart::Root { index: 0, key }
                    } else {
                        CodePart::KeyRepeat { key }
                    }
                })
                .collect(),
            CharKind::Radical if is_single_stroke(self.ch) => keys
                .iter()
                .enumerate()
                .map(|(i, &key)| match i {
                    0 => CodePart::Root { index: 0, key },
                    1 => CodePart::Stroke {
                        key,
                        stroke: "本笔",
                    },
                    _ => CodePart::Stroke {
                        key,
                        stroke: "补 L",
                    },
                })
                .collect(),
            CharKind::Radical => keys
                .iter()
                .enumerate()
                .map(|(i, &key)| match i {
                    0 => CodePart::Root { index: 0, key },
                    _ => {
                        let label = match (i, keys.len()) {
                            (1, _) => "首笔",
                            (2, 4) => "次笔",
                            _ => "末笔",
                        };
                        CodePart::Stroke { key, stroke: label }
                    }
                })
                .collect(),
            CharKind::Compound => {
                let n = self.roots.len();
                let mut parts = Vec::new();
                if n >= 4 {
                    for (slot, index) in [0, 1, 2, n - 1].into_iter().enumerate() {
                        parts.push(CodePart::Root {
                            index,
                            key: keys.get(slot).copied().unwrap_or('?'),
                        });
                    }
                } else {
                    for (index, root) in self.roots.iter().enumerate() {
                        parts.push(CodePart::Root {
                            index,
                            key: root.key,
                        });
                    }
                    if let Some(key) = self.ident {
                        let (stroke, shape) = ident_meaning(key);
                        parts.push(CodePart::Ident { key, stroke, shape });
                    }
                }
                parts
            }
        }
    }

    /// 计入编码的字根下标（≥4 个字根时只取前三和末一）。
    pub fn coded_root_indexes(&self) -> Vec<usize> {
        self.code_parts()
            .iter()
            .filter_map(|p| match p {
                CodePart::Root { index, .. } => Some(*index),
                _ => None,
            })
            .collect()
    }
}

fn parse_roots(col: &str) -> (Vec<Root>, bool) {
    let (verified, body) = match col.strip_prefix('!') {
        Some(rest) => (false, rest),
        None => (true, col),
    };
    let roots = body
        .split(' ')
        .filter_map(|tok| {
            let mut fields = tok.splitn(3, ':');
            let glyph = fields.next()?;
            let key = fields.next()?.chars().next()?;
            let font_glyph = fields.next().and_then(|g| g.chars().next());
            Some(Root {
                glyph: glyph.to_string(),
                key,
                font_glyph,
            })
        })
        .collect();
    (roots, verified)
}

/// 末笔识别码：键所在区为末笔笔画，位为字型。
pub fn ident_meaning(key: char) -> (&'static str, &'static str) {
    const TABLE: [(&str, &str); 5] = [
        ("GFD", "横"),
        ("HJK", "竖"),
        ("TRE", "撇"),
        ("YUI", "捺"),
        ("NBV", "折"),
    ];
    for (keys, stroke) in TABLE {
        if let Some(pos) = keys.find(key) {
            let shape = ["左右型", "上下型", "杂合型"][pos];
            return (stroke, shape);
        }
    }
    ("", "")
}

/// 内存中的字典，启动时从 SQLite 读入。
#[derive(Default)]
pub struct Dictionary {
    entries: HashMap<char, Entry>,
}

impl Dictionary {
    pub fn from_entries(entries: impl IntoIterator<Item = Entry>) -> Self {
        Self {
            entries: entries.into_iter().map(|e| (e.ch, e)).collect(),
        }
    }

    pub fn parse_tsv(tsv: &str) -> Vec<Entry> {
        tsv.lines().filter_map(Entry::parse_line).collect()
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

    fn dict() -> Dictionary {
        Dictionary::from_entries(Dictionary::parse_tsv(DICT_TSV))
    }

    #[test]
    fn common_chars_are_present() {
        let d = dict();
        assert!(d.len() > 9000);
        for ch in "我们的是学习五笔照键盘".chars() {
            assert!(d.contains(ch), "{ch}");
        }
    }

    #[test]
    fn compound_with_ident() {
        let d = dict();
        let e = d.get('的').unwrap();
        assert_eq!(e.full_code, "RQYY");
        assert_eq!(e.kind, CharKind::Compound);
        assert_eq!(
            e.roots.iter().map(|r| r.glyph.as_str()).collect::<Vec<_>>(),
            ["白", "勹", "丶"]
        );
        assert_eq!(e.ident, Some('Y'));
        let parts = e.code_parts();
        assert_eq!(parts.len(), 4);
        assert_eq!(
            parts[3],
            CodePart::Ident {
                key: 'Y',
                stroke: "捺",
                shape: "左右型"
            }
        );
    }

    #[test]
    fn four_or_more_roots_take_first_three_and_last() {
        let d = dict();
        let e = d.get('键').unwrap();
        assert_eq!(e.full_code, "QVFP");
        assert_eq!(e.roots.len(), 5);
        assert_eq!(e.coded_root_indexes(), vec![0, 1, 2, 4]);
    }

    #[test]
    fn root_font_glyphs() {
        let d = dict();
        let e = d.get('的').unwrap();
        assert_eq!(e.code_glyphs.len(), 4);
        assert!(e.roots.iter().all(|r| r.font_glyph.is_some()));
        for key in "QWERTYUIOPASDFGHJKLXCVBNM".chars() {
            assert!(key_roots(key).len() >= 3, "{key}");
        }
    }

    #[test]
    fn key_name_and_radical() {
        let d = dict();
        assert_eq!(d.get('王').unwrap().kind, CharKind::KeyName);
        let five = d.get('五').unwrap();
        assert_eq!(five.kind, CharKind::Radical);
        assert!(matches!(
            five.code_parts()[1],
            CodePart::Stroke {
                stroke: "首笔", ..
            }
        ));
        let one = d.get('一').unwrap();
        assert_eq!(one.full_code, "GGLL");
        assert!(matches!(
            one.code_parts()[3],
            CodePart::Stroke {
                stroke: "补 L", ..
            }
        ));
    }

    #[test]
    fn line_round_trip() {
        let d = dict();
        let e = d.get('照').unwrap();
        let line = format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            e.ch,
            e.full_code,
            e.short_codes.join(","),
            e.kind.as_str(),
            e.roots_column(),
            e.ident.map(String::from).unwrap_or_default(),
            e.code_glyphs.iter().collect::<String>()
        );
        assert_eq!(Entry::parse_line(&line).as_ref(), Some(e));
    }
}
