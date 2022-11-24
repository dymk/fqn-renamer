use crate::{
    fqcn::Fqcn,
    matched_file::{MatchType, MatchedFile},
};

pub fn process_matched_file_fqcn(
    fqcn: &Fqcn,
    mut matched_files: Vec<MatchedFile>,
) -> Vec<MatchedFile> {
    let fqcn_packg = fqcn.package();
    let fqcn_value = fqcn.value();
    let fqcn_ident = fqcn.ident();

    matched_files.retain_mut(|matched_file| {
        let mut saw_package = false;
        let mut saw_import = false;
        let mut saw_ident = false;

        matched_file.matches_mut().for_each(|m| {
            if m.value.starts_with("import ") {
                saw_import = true;
            }

            const PACKAGE: &str = "package ";

            let start = m.match_range.start;

            if m.value.starts_with(PACKAGE) {
                m.match_range.start = PACKAGE.len();
                m.match_range.end = PACKAGE.len() + fqcn_packg.len();
                m.match_type = MatchType::Package;
                saw_package = true;
            } else if let Some(idx) = m.value[start..].find(fqcn_value) {
                m.match_range.start = start + idx;
                m.match_range.end = start + idx + fqcn_value.len();
                m.match_type = MatchType::FqcnFull;
                saw_ident = true;
            } else if let Some(idx) = m.value[start..].find(fqcn_ident) {
                m.match_range.start = start + idx;
                m.match_range.end = start + idx + fqcn_ident.len();
                m.match_type = MatchType::FqcnIdent;
                saw_ident = true;
            }
        });

        (saw_package || saw_import) && saw_ident
    });

    matched_files
}

#[cfg(test)]
mod test {
    use crate::{
        fqcn::Fqcn,
        matched_file::{Line, Match, MatchType, MatchedFile},
    };

    use super::process_matched_file_fqcn;

    #[test]
    fn test_works() {
        let fqcn = Fqcn::new("foo.bar.Baz").unwrap();
        assert_eq!("foo.bar", fqcn.package());

        let matches = vec![
            MatchedFile {
                file_path: "foo/bar/Baz.java".into(),
                lines: vec![
                    Line::Match(Match {
                        match_type: MatchType::Arbitrary,
                        line_num: 3,
                        match_range: 0..16,
                        value: "package foo.bar;".into(),
                    }),
                    Line::Match(Match {
                        match_type: MatchType::Arbitrary,
                        line_num: 4,
                        match_range: 6..(6 + 3),
                        value: "class Baz {};".into(),
                    }),
                ],
            },
            MatchedFile {
                file_path: "foo/Quux.java".into(),
                lines: vec![
                    Line::Match(Match {
                        match_type: MatchType::Arbitrary,
                        line_num: 2,
                        match_range: 7..(7 + 11),
                        value: "import foo.bar.Baz;".into(),
                    }),
                    Line::Match(Match {
                        match_type: MatchType::Arbitrary,
                        line_num: 8,
                        match_range: 3..(3 + 3),
                        value: "   Baz myBaz = new Baz()".into(),
                    }),
                    Line::Match(Match {
                        match_type: MatchType::Arbitrary,
                        line_num: 8,
                        match_range: 5..(5 + 11),
                        value: " new foo.bar.Baz(1, 2);".into(),
                    }),
                ],
            },
        ];

        let matches = process_matched_file_fqcn(&fqcn, matches);
        assert_eq!(matches.len(), 2);

        assert_eq!("foo/bar/Baz.java", matches[0].file_path);
        assert_eq!(matches[0].lines.len(), 2);

        {
            let m = cast_to_match(&matches[0].lines[0]);
            assert_eq!(MatchType::Package, m.match_type);
            assert_bma("package ", "foo.bar", ";", m);
        }

        {
            let m = cast_to_match(&matches[0].lines[1]);
            assert_eq!(MatchType::FqcnIdent, m.match_type);
            assert_bma("class ", "Baz", " {};", m);
        }

        assert_eq!("foo/Quux.java", matches[1].file_path);
        assert_eq!(matches[1].lines.len(), 3);

        {
            let m = cast_to_match(&matches[1].lines[0]);
            assert_eq!(MatchType::FqcnFull, m.match_type);
            assert_bma("import ", "foo.bar.Baz", ";", m);
        }

        {
            let m = cast_to_match(&matches[1].lines[1]);
            assert_eq!(MatchType::FqcnIdent, m.match_type);
            assert_bma("   ", "Baz", " myBaz = new Baz()", m);
        }

        {
            let m = cast_to_match(&matches[1].lines[2]);
            assert_eq!(MatchType::FqcnFull, m.match_type);
            assert_bma(" new ", "foo.bar.Baz", "(1, 2);", m);
        }
    }

    fn assert_bma(before: &str, middle: &str, after: &str, m: &Match) {
        assert_eq!(before, m.before());
        assert_eq!(middle, m.middle());
        assert_eq!(after, m.after());
    }

    fn cast_to_match(line: &Line) -> &Match {
        match line {
            Line::Match(m) => m,
            _ => panic!(),
        }
    }
}
