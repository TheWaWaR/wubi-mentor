//! 练习进程：逐字比对输入法上屏的文字，统计速度、错误与停顿。

use std::collections::HashMap;

/// 两次上屏之间超过这个时间就当作走神，不计入用时。
pub const IDLE_CAP_MS: i64 = 15_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// 一次打对。
    Clean,
    /// 打错过，最终打对。
    Fixed,
    /// 跳过。
    Skipped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Correct { index: usize, ch: char, ms: i64, errors: u32 },
    Wrong { index: usize, expected: char, got: char },
    Finished,
}

pub struct Practice {
    chars: Vec<char>,
    targets: Vec<bool>,
    pos: usize,
    marks: HashMap<usize, Mark>,
    /// 当前字已经打错的次数和最近一次打成了什么。
    errors_here: u32,
    last_wrong: Option<char>,
    last_event_ms: Option<i64>,
    session: SessionTally,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionTally {
    pub started_at: Option<i64>,
    pub last_at: i64,
    pub active_ms: i64,
    pub chars: i64,
    pub missed: i64,
}

impl SessionTally {
    pub fn speed(&self) -> f64 {
        if self.active_ms < 1000 { 0.0 } else { self.chars as f64 * 60_000.0 / self.active_ms as f64 }
    }

    pub fn accuracy(&self) -> f64 {
        if self.chars == 0 { 1.0 } else { 1.0 - self.missed as f64 / self.chars as f64 }
    }
}

impl Practice {
    /// `is_target` 决定哪些字需要打（字典里有的汉字），其余字自动跳过。
    pub fn new(content: &str, position: usize, is_target: impl Fn(char) -> bool) -> Self {
        let chars: Vec<char> = content.chars().collect();
        let targets = chars.iter().map(|&c| is_target(c)).collect();
        let mut p = Self {
            chars,
            targets,
            pos: position,
            marks: HashMap::new(),
            errors_here: 0,
            last_wrong: None,
            last_event_ms: None,
            session: SessionTally::default(),
        };
        p.pos = p.next_target_from(position.min(p.chars.len()));
        p
    }

    fn next_target_from(&self, mut i: usize) -> usize {
        while i < self.chars.len() && !self.targets[i] {
            i += 1;
        }
        i
    }

    pub fn chars(&self) -> &[char] {
        &self.chars
    }

    pub fn is_target(&self, index: usize) -> bool {
        self.targets.get(index).copied().unwrap_or(false)
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn current(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    pub fn is_finished(&self) -> bool {
        self.pos >= self.chars.len()
    }

    pub fn mark(&self, index: usize) -> Option<Mark> {
        self.marks.get(&index).copied()
    }

    pub fn errors_here(&self) -> u32 {
        self.errors_here
    }

    pub fn last_wrong(&self) -> Option<char> {
        self.last_wrong
    }

    pub fn session(&self) -> SessionTally {
        self.session
    }

    pub fn target_count(&self) -> usize {
        self.targets.iter().filter(|t| **t).count()
    }

    pub fn targets_done(&self) -> usize {
        self.targets[..self.pos.min(self.targets.len())].iter().filter(|t| **t).count()
    }

    /// 自上次上屏以来的毫秒数（用于“停顿太久就提示”）。
    pub fn idle_ms(&self, now: i64) -> Option<i64> {
        self.last_event_ms.map(|t| now - t)
    }

    fn tick(&mut self, now: i64) -> i64 {
        let ms = match self.last_event_ms {
            Some(t) => (now - t).clamp(0, IDLE_CAP_MS),
            None => 0,
        };
        self.last_event_ms = Some(now);
        if self.session.started_at.is_none() {
            self.session.started_at = Some(now);
        }
        self.session.last_at = now;
        self.session.active_ms += ms;
        ms
    }

    /// 处理一段上屏文字（可能是一个词组）。遇到第一个错字就停止。
    pub fn input(&mut self, text: &str, now: i64) -> Vec<Outcome> {
        let mut out = Vec::new();
        let mut elapsed = None;
        for got in text.chars() {
            let Some(expected) = self.current() else { break };
            if got.is_whitespace() {
                continue;
            }
            // 同一次上屏的多个字平分用时。
            let ms = *elapsed.get_or_insert_with(|| self.tick(now));
            if got == expected {
                let errors = self.errors_here;
                self.marks.insert(self.pos, if errors == 0 { Mark::Clean } else { Mark::Fixed });
                self.session.chars += 1;
                self.session.missed += i64::from(errors > 0);
                out.push(Outcome::Correct { index: self.pos, ch: expected, ms, errors });
                elapsed = Some(0);
                self.advance();
                if self.is_finished() {
                    out.push(Outcome::Finished);
                    break;
                }
            } else if !(is_cjk(got) || got.is_alphanumeric()) {
                // 标点符号等非练习字：忽略。
                continue;
            } else {
                self.errors_here += 1;
                self.last_wrong = Some(got);
                out.push(Outcome::Wrong { index: self.pos, expected, got });
                break;
            }
        }
        out
    }

    fn advance(&mut self) {
        self.errors_here = 0;
        self.last_wrong = None;
        self.pos = self.next_target_from(self.pos + 1);
    }

    /// 跳过当前字，记为未掌握。
    pub fn skip(&mut self) -> Option<char> {
        let ch = self.current()?;
        self.marks.insert(self.pos, Mark::Skipped);
        self.advance();
        Some(ch)
    }

    /// 跳到指定位置（之后的标记清除）。
    pub fn jump_to(&mut self, index: usize) {
        let index = self.next_target_from(index.min(self.chars.len()));
        self.marks.retain(|&i, _| i < index);
        self.pos = index;
        self.errors_here = 0;
        self.last_wrong = None;
    }

    /// 结束本次练习，返回统计并开始新的一轮计数。
    pub fn take_session(&mut self) -> Option<SessionTally> {
        let s = std::mem::take(&mut self.session);
        self.last_event_ms = None;
        (s.chars > 0).then_some(s)
    }
}

pub fn is_cjk(ch: char) -> bool {
    matches!(ch as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x3134F)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn practice(text: &str) -> Practice {
        Practice::new(text, 0, is_cjk)
    }

    #[test]
    fn skips_punctuation_and_whitespace() {
        let mut p = practice("，我 们。");
        assert_eq!(p.current(), Some('我'));
        assert_eq!(p.target_count(), 2);
        let out = p.input("我", 1000);
        assert!(matches!(out[0], Outcome::Correct { ch: '我', .. }));
        assert_eq!(p.current(), Some('们'));
        let out = p.input("们", 2000);
        assert_eq!(out.last(), Some(&Outcome::Finished));
        assert!(p.is_finished());
    }

    #[test]
    fn phrase_input_advances_multiple() {
        let mut p = practice("我们学习");
        let out = p.input("我们", 0);
        assert_eq!(out.len(), 2);
        assert_eq!(p.current(), Some('学'));
    }

    #[test]
    fn wrong_char_stays_and_counts() {
        let mut p = practice("我们");
        let out = p.input("找", 0);
        assert_eq!(out, vec![Outcome::Wrong { index: 0, expected: '我', got: '找' }]);
        assert_eq!(p.current(), Some('我'));
        assert_eq!(p.errors_here(), 1);
        let out = p.input("我", 500);
        assert!(matches!(out[0], Outcome::Correct { errors: 1, .. }));
        assert_eq!(p.mark(0), Some(Mark::Fixed));
        assert_eq!(p.session().missed, 1);
    }

    #[test]
    fn typed_punctuation_is_ignored() {
        let mut p = practice("我，们");
        p.input("我，", 0);
        assert_eq!(p.current(), Some('们'));
        assert_eq!(p.errors_here(), 0);
    }

    #[test]
    fn idle_time_is_capped() {
        let mut p = practice("一二三");
        p.input("一", 0);
        p.input("二", 1_000_000);
        assert_eq!(p.session().active_ms, IDLE_CAP_MS);
    }

    #[test]
    fn resume_from_saved_position() {
        let p = Practice::new("我们，学习", 2, is_cjk);
        assert_eq!(p.position(), 3);
        assert_eq!(p.targets_done(), 2);
    }
}
