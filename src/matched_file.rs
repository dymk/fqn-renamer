use std::ops::Range;

#[derive(Debug, Default)]
pub struct MatchedFile {
    pub file_path: String,
    pub lines: Vec<Line>,
}

impl MatchedFile {
    pub fn matches_mut(&mut self) -> impl Iterator<Item = &mut Match> {
        self.lines.iter_mut().filter_map(|line| match line {
            Line::Match(m) => Some(m),
            _ => None,
        })
    }

    pub fn matches(&self) -> impl Iterator<Item = &Match> {
        self.lines.iter().filter_map(|line| match line {
            Line::Match(m) => Some(m),
            _ => None,
        })
    }

    pub fn replace<R: Fn(&str) -> String>(&self, replacer: &R) -> MatchedFile {
        MatchedFile {
            file_path: self.file_path.clone(),
            lines: self
                .lines
                .iter()
                .map(|line| line.replace(replacer))
                .collect(),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MatchType {
    Arbitrary,
    Package,
    FqcnIdent,
    FqcnFull,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Match {
    pub match_type: MatchType,
    pub line_num: usize,
    pub match_range: Range<usize>,
    pub value: String,
}
impl Match {
    pub fn replace<R: Fn(&str) -> String>(&self, replacer: &R) -> Match {
        let new_value = format!(
            "{}{}{}",
            self.before(),
            replacer(self.middle()),
            self.after()
        );
        let delta = new_value.len() as i32 - self.value.len() as i32;
        let new_end = self.match_range.end as i32 + delta;

        Match {
            match_type: self.match_type,
            line_num: self.line_num,
            value: new_value,
            match_range: (self.match_range.start..(new_end as usize)),
        }
    }

    pub fn before(&self) -> &str {
        &self.value[..self.match_range.start]
    }
    pub fn middle(&self) -> &str {
        &self.value[self.match_range.clone()]
    }
    pub fn after(&self) -> &str {
        &self.value[self.match_range.end..]
    }
}

#[derive(Debug)]
pub enum Line {
    Context { line_num: usize, value: String },

    Match(Match),
}

impl Line {
    pub fn line_num(&self) -> usize {
        *match self {
            Line::Context { line_num, .. } => line_num,
            Line::Match(Match { line_num, .. }) => line_num,
        }
    }

    pub fn replace<R: Fn(&str) -> String>(&self, replacer: &R) -> Line {
        match self {
            Line::Context { line_num, value } => Line::Context {
                line_num: *line_num,
                value: value.clone(),
            },
            Line::Match(m) => Line::Match(m.replace(replacer)),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::matched_file::MatchType;

    use super::Match;

    #[test]
    fn test_works() {
        let m = Match {
            match_type: MatchType::Arbitrary,
            line_num: 10,
            match_range: 3..6,
            value: "0123456789".to_owned(),
        };
        assert_eq!("012", m.before());
        assert_eq!("345", m.middle());
        assert_eq!("6789", m.after());

        let n = m.replace(&|_| "bar".to_owned());
        assert_eq!("012", n.before());
        assert_eq!("bar", n.middle());
        assert_eq!("6789", n.after());

        let o = m.replace(&|_| "foosmaz".to_owned());
        assert_eq!("012", o.before());
        assert_eq!("foosmaz", o.middle());
        assert_eq!("6789", o.after());
    }
}
