//! 86 版五笔拆字数据：字根、全码、简码与识别码。

use std::collections::HashMap;

/// Built by `scripts/build_dict.py`; seeded into SQLite on first run.
pub const DICT_TSV: &str = include_str!("../data/wubi86.tsv");
/// Bump when `data/wubi86.tsv` changes so existing databases reseed.
pub const DICT_VERSION: &str = "1";

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

/// 每个键上的全部字根（86 版，不含繁体），按字根表的顺序，最常用的在前。
/// 没有 Unicode 字形的变形写法用文字描述（如“羊底”“母框”）。
pub fn key_roots(key: char) -> &'static [&'static str] {
    match key {
        'G' => &["王", "一", "五", "戋", "龶"],
        'F' => &["土", "士", "二", "干", "十", "寸", "雨", "革底"],
        'D' => &[
            "大", "犬", "三", "古", "石", "厂", "ナ", "丆", "镸", "羊底", "長头",
        ],
        'S' => &["木", "丁", "西", "覀", "朩"],
        'A' => &[
            "工", "戈", "弋", "七", "匚", "匸", "艹", "廾", "廿", "卄", "龷",
        ],
        'H' => &["目", "止", "卜", "⺊", "上", "丨", "龰", "且", "虍", "具头"],
        'J' => &["日", "曰", "早", "虫", "刂", "〢", "竖川"],
        'K' => &["口", "川"],
        'L' => &["田", "甲", "囗", "四", "罒", "皿", "车", "力", "四竖"],
        'M' => &["山", "由", "贝", "冂", "几", "𠘨", "骨头"],
        'T' => &["禾", "竹", "⺮", "丿", "𠂉", "彳", "夂", "攵", "牛底"],
        'R' => &["白", "手", "扌", "斤", "𠂆", "牛前", "气头"],
        'E' => &[
            "月", "彡", "乃", "用", "冃", "豕", "衣底", "爫", "舟底", "豸头", "家底",
        ],
        'W' => &["人", "亻", "八", "𠆢", "癶", "祭头"],
        'Q' => &[
            "金",
            "钅",
            "勹",
            "儿",
            "夕",
            "犭",
            "乂",
            "⺈",
            "鱼头",
            "氏无七",
        ],
        'Y' => &["言", "讠", "文", "方", "广", "丶", "亠", "主头"],
        'U' => &["立", "辛", "六", "门", "疒", "丬", "冫", "丷", "䒑", "立头"],
        'I' => &["水", "氵", "氺", "小", "⺌", "⺍", "兴头", "水侧"],
        'O' => &["火", "灬", "米", "业头", "小倒"],
        'P' => &["之", "辶", "廴", "宀", "冖", "礻", "衤"],
        'N' => &[
            "已", "己", "巳", "乙", "尸", "心", "忄", "⺗", "羽", "コ", "乚", "乛",
        ],
        'B' => &["子", "孑", "了", "耳", "也", "卩", "阝", "凵", "巜"],
        'V' => &["女", "刀", "九", "臼", "彐", "巛"],
        'C' => &["又", "巴", "马", "厶", "ス", "龴"],
        'X' => &["纟", "糹", "幺", "弓", "匕", "母框"],
        _ => &[],
    }
}

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
        Some(Self {
            ch,
            full_code,
            short_codes,
            kind,
            roots,
            ident,
            verified,
        })
    }

    pub fn roots_column(&self) -> String {
        let body = self
            .roots
            .iter()
            .map(|r| format!("{}:{}", r.glyph, r.key))
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
            let (glyph, key) = tok.rsplit_once(':')?;
            Some(Root {
                glyph: glyph.to_string(),
                key: key.chars().next()?,
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
            "{}\t{}\t{}\t{}\t{}\t{}",
            e.ch,
            e.full_code,
            e.short_codes.join(","),
            e.kind.as_str(),
            e.roots_column(),
            e.ident.map(String::from).unwrap_or_default()
        );
        assert_eq!(Entry::parse_line(&line).as_ref(), Some(e));
    }
}
