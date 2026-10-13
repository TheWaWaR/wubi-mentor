//! 输入方案：86 版五笔或第五代仓颉。字典、拆字卡、键盘和练习统计都跟着方案走。

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scheme {
    Wubi86,
    Cangjie5,
}

impl Scheme {
    pub const ALL: [Scheme; 2] = [Self::Wubi86, Self::Cangjie5];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wubi86 => "wubi86",
            Self::Cangjie5 => "cangjie5",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "cangjie5" => Self::Cangjie5,
            _ => Self::Wubi86,
        }
    }

    /// 侧栏和菜单里的名字。
    pub fn label(self) -> &'static str {
        match self {
            Self::Wubi86 => "五笔 86 版",
            Self::Cangjie5 => "仓颉第五代",
        }
    }

    /// 侧栏图标里的一个字。
    pub fn logo(self) -> &'static str {
        match self {
            Self::Wubi86 => "五",
            Self::Cangjie5 => "倉",
        }
    }

    /// 系统输入法的叫法，用于提示。
    pub fn ime(self) -> &'static str {
        match self {
            Self::Wubi86 => "五笔",
            Self::Cangjie5 => "仓颉",
        }
    }

    /// 键位分区：五笔是横竖撇捺折五区，仓颉是哲理、笔画、人体、字形四类（X 难字键不归类）。
    pub fn zone_of(self, key: char) -> Option<usize> {
        let zones: &[&str] = match self {
            Self::Wubi86 => &["GFDSA", "HJKLM", "TREWQ", "YUIOP", "NBVCX"],
            Self::Cangjie5 => &["ABCDEFG", "HIJKLMN", "OPQR", "STUVWY"],
        };
        zones.iter().position(|z| z.contains(key))
    }

    pub fn zone_names(self) -> &'static [&'static str] {
        match self {
            Self::Wubi86 => &["横区", "竖区", "撇区", "捺区", "折区"],
            Self::Cangjie5 => &["哲理类", "笔画类", "人体类", "字形类"],
        }
    }
}
