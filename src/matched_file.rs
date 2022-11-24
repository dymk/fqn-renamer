use std::ops::Range;

use itertools::Itertools;

#[derive(Debug, Default, Eq, PartialEq)]
pub struct MatchedFile {
    file_path: String,
    lines: Vec<Line>,
}

impl MatchedFile {
    pub fn new<S: Into<String>, VL: Into<Vec<Line>>>(file_path: S, lines: VL) -> Self {
        MatchedFile {
            file_path: file_path.into(),
            lines: lines.into(),
        }
    }

    pub fn file_path(&self) -> &str {
        self.file_path.as_str()
    }

    pub fn lines_mut(&mut self) -> impl ExactSizeIterator<Item = &mut Line> {
        self.lines.iter_mut()
    }

    pub fn lines(&self) -> impl ExactSizeIterator<Item = &Line> {
        self.lines.iter()
    }

    pub fn replace<R: Fn(&str) -> S, S: Into<String>>(&self, replacer: R) -> MatchedFile {
        MatchedFile {
            file_path: self.file_path.clone(),
            lines: self
                .lines
                .iter()
                .map(|line| line.replace(&replacer))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Line {
    num: usize,
    value: String,
    submatches: Vec<Range<usize>>,
}

impl Line {
    pub fn new<S: Into<String>>(num: usize, value: S, submatches: Vec<Range<usize>>) -> Self {
        let ret = Self {
            num,
            value: value.into(),
            submatches,
        };
        ret.check_invariants();
        ret
    }

    pub fn num(&self) -> usize {
        self.num
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn num_submatches(&self) -> usize {
        self.submatches.len()
    }

    pub fn replace<R: Fn(&str) -> S, S: Into<String>>(&self, replacer: R) -> Self {
        let mut new_value = String::new();
        let mut new_submatches = vec![];
        let mut pos = 0;

        for (is_match, part) in self.iter() {
            if is_match {
                let replaced = replacer(part).into();
                if replaced.is_empty() {
                    // skip if empty
                } else {
                    new_value += &replaced;
                    new_submatches.push(pos..pos + replaced.len());
                    pos += replaced.len()
                }
            } else {
                new_value += part;
                pos += part.len();
            }
        }

        Line::new(self.num, new_value, new_submatches)
    }

    // adjust the range that each submatch covers, e.g. so we can change
    // `[package foo.bar];` to be `package [foo.bar];`
    pub fn adjust_submatches<A: FnMut(&str) -> Range<usize>>(&mut self, mut adjuster: A) {
        self.submatches.retain_mut(|submatch| {
            let sm_value = &self.value[submatch.clone()];
            let new_range = adjuster(sm_value);
            submatch.start += new_range.start;
            submatch.end = submatch.start + new_range.len();

            // retain only if the submatch isn't empty
            !submatch.is_empty()
        });
        self.check_invariants();
    }

    pub fn iter(&self) -> impl Iterator<Item = (bool, &str)> {
        LinePartsIter::from_line(self)
    }

    fn check_invariants(&self) {
        for (idx, submatch) in self.submatches.iter().enumerate() {
            if submatch.is_empty() {
                panic!("must not be zero len: {:?}@{}", submatch, idx);
            }
        }

        for (idx, (a, b)) in self.submatches.iter().tuple_windows().enumerate() {
            if a.end > b.start {
                panic!("must not overlap: {:?}, {:?} @ {}", a, b, idx);
            }
        }
    }
}

struct LinePartsIter<'a> {
    value: &'a str,
    pos: usize,
    submatches: &'a [Range<usize>],
}

impl<'a> LinePartsIter<'a> {
    fn from_line(line: &'a Line) -> LinePartsIter<'a> {
        Self {
            value: line.value(),
            pos: 0,
            submatches: line.submatches.as_slice(),
        }
    }
}

impl<'a> Iterator for LinePartsIter<'a> {
    type Item = (bool, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        let mut shift_submatches = false;
        let pos = self.pos;
        let mut next_pos = pos;

        let ret = if let Some(submatch) = self.submatches.first() {
            // if there is a submatch, check if we're reading from its start
            if submatch.start == pos {
                // if so, consume the submatch and go to its end
                shift_submatches = true;
                next_pos = submatch.end;
                Some((true, &self.value[submatch.clone()]))
            } else {
                // if not, read up to the submatch
                next_pos = submatch.start;
                Some((false, &self.value[pos..submatch.start]))
            }
        } else {
            // no submatches, consume the remainder of the value
            if pos == self.value.len() {
                None
            } else {
                next_pos = self.value.len();
                Some((false, &self.value[pos..]))
            }
        };

        if shift_submatches {
            self.submatches = &self.submatches[1..];
        }

        self.pos = next_pos;
        ret
    }
}

#[cfg(test)]
mod test {
    use std::ops::Range;

    use itertools::assert_equal;

    use super::Line;

    #[test]
    fn test_line_iter() {
        assert_equal(
            [(false, "0123456789")],
            new_line("0123456789", vec![]).iter().take(100),
        );

        assert_equal(
            [(true, "0123456789")],
            new_line("0123456789", vec![0..10]).iter().take(100),
        );

        assert_equal(
            [(false, "01234"), (true, "56789")],
            new_line("0123456789", vec![5..10]).iter().take(100),
        );

        assert_equal(
            [(true, "01234"), (false, "56789")],
            new_line("0123456789", vec![0..5]).iter().take(100),
        );

        assert_equal(
            [(true, "01234"), (false, "567"), (true, "89")],
            new_line("0123456789", vec![0..5, 8..10]).iter().take(100),
        );

        assert_equal(
            [(false, "01234"), (true, "567"), (false, "89")],
            new_line("0123456789", vec![5..8]).iter().take(100),
        );

        assert_equal(
            [(false, "012"), (true, "345"), (false, "67"), (true, "89")],
            new_line("0123456789", vec![3..6, 8..10]).iter().take(100),
        );
    }

    #[test]
    fn test_adjust_submatches() {
        // [foo ][bar baz] =>
        // [foo] bar [baz]
        let mut line = new_line("foo bar baz", vec![0..4, 4..11]);

        assert_equal([(true, "foo "), (true, "bar baz")], line.iter().take(100));

        line.adjust_submatches(|submatch| {
            if submatch == "foo " {
                0..3
            } else if submatch == "bar baz" {
                4..7
            } else {
                0..submatch.len()
            }
        });

        assert_equal(
            [(true, "foo"), (false, " bar "), (true, "baz")],
            line.iter().take(100),
        );
    }

    #[test]
    fn test_drop_empty_submatches() {
        let mut line = new_line("foo bar baz", vec![0..4, 4..11]);
        line.adjust_submatches(|submatch| if submatch == "foo " { 0..3 } else { 0..0 });

        assert_equal([(true, "foo"), (false, " bar baz")], line.iter().take(100));
    }

    #[test]
    fn test_replace_shortens() {
        let line = new_line("0123456789", vec![2..6])
            .replace(|substr| if substr == "2345" { "." } else { substr }.to_owned());

        assert_eq!("01.6789", line.value());
        assert_equal(
            [(false, "01"), (true, "."), (false, "6789")],
            line.iter().take(100),
        );
    }

    #[test]
    fn test_replace_lengthens() {
        let line = new_line("0123456789", vec![2..6]).replace(|substr| {
            if substr == "2345" {
                "foobarbaz"
            } else {
                substr
            }
            .to_owned()
        });

        assert_eq!("01foobarbaz6789", line.value());
        assert_equal(
            [(false, "01"), (true, "foobarbaz"), (false, "6789")],
            line.iter().take(100),
        );
    }

    #[test]
    fn test_replace_same_len() {
        let line = new_line("0123456789", vec![2..6])
            .replace(|substr| if substr == "2345" { "smaz" } else { substr }.to_owned());

        assert_eq!("01smaz6789", line.value());
        assert_equal(
            [(false, "01"), (true, "smaz"), (false, "6789")],
            line.iter().take(100),
        );
    }

    #[test]
    fn test_replace_drops_empty() {
        let line = new_line("0123456789", vec![2..6])
            .replace(|substr| if substr == "2345" { "" } else { substr }.to_owned());

        assert_eq!("016789", line.value());
        assert_equal([(false, "016789")], line.iter().take(100));
    }

    fn new_line(value: &str, matches: Vec<Range<usize>>) -> Line {
        Line::new(0, value, matches)
    }

    // #[test]
    // fn test_works() {
    //     let m = MatchedLine {
    //         match_type: MatchType::Arbitrary,
    //         line_num: 10,
    //         match_range: 3..6,
    //         value: "0123456789".to_owned(),
    //     };
    //     assert_eq!("012", m.before());
    //     assert_eq!("345", m.middle());
    //     assert_eq!("6789", m.after());

    //     let n = m.replace(&|_| "bar".to_owned());
    //     assert_eq!("012", n.before());
    //     assert_eq!("bar", n.middle());
    //     assert_eq!("6789", n.after());

    //     let o = m.replace(&|_| "foosmaz".to_owned());
    //     assert_eq!("012", o.before());
    //     assert_eq!("foosmaz", o.middle());
    //     assert_eq!("6789", o.after());
    // }
}
