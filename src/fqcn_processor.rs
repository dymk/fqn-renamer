use crate::{fqcn::Fqcn, matched_file::MatchedFile};

const PACKAGE: &str = "package ";
const IMPORT: &str = "import ";

pub fn process_matched_file_fqcn(
    fqcn: &Fqcn,
    mut matched_files: Vec<MatchedFile>,
) -> Vec<MatchedFile> {
    let fqcn_value = fqcn.value();
    let fqcn_packg = fqcn.package();
    let fqcn_ident = fqcn.ident();

    matched_files.retain_mut(|matched_file| {
        let mut saw_package = false;
        let mut saw_import = false;
        let mut saw_ident = false;
        let mut saw_fqcn = false;

        matched_file.lines_mut().for_each(|line| {
            let line_value = line.value();

            if line_value.starts_with(IMPORT) && line_value.contains(fqcn_value) {
                saw_import = true;
            } else if line_value.starts_with(PACKAGE) && line_value.contains(fqcn_packg) {
                saw_package = true;
            }

            line.adjust_submatches(|submatch| {
                // println!("adjusting: {} -> {}", submatch, &submatch[ret.clone()]);

                if let Some(idx) = submatch.find(fqcn_value) {
                    saw_fqcn = true;
                    idx..idx + fqcn_value.len()
                } else if let Some(idx) = submatch.find(fqcn_packg) {
                    idx..idx + fqcn_packg.len()
                } else if let Some(idx) = submatch.find(fqcn_ident) {
                    saw_ident = true;
                    idx..idx + fqcn_ident.len()
                } else {
                    0..submatch.len()
                }
            });
        });

        // println!("saw fqcn: {}", saw_fqcn);
        // println!("saw import: {}", saw_import);
        saw_fqcn || saw_import || (saw_package && saw_ident)
    });

    matched_files
}

#[cfg(test)]
mod test {
    use itertools::assert_equal;

    use crate::{
        fqcn::Fqcn,
        matched_file::{Line, MatchedFile},
    };

    use super::process_matched_file_fqcn;

    #[test]
    fn test_works() {
        let fqcn = Fqcn::new("foo.bar.Baz").unwrap();

        let matches = vec![
            MatchedFile::new(
                "foo/bar/Baz.java",
                vec![
                    Line::new(3, "package foo.bar", vec![0..15]),
                    Line::new(4, "class Baz {};", vec![6..(6 + 3)]),
                ],
            ),
            MatchedFile::new(
                "foo/Quux.java",
                vec![
                    Line::new(2, "import foo.bar.Baz", vec![7..(7 + 11)]),
                    Line::new(
                        8,
                        "   Baz myBaz = new Baz()",
                        vec![3..(3 + 3), 19..(19 + 3)],
                    ),
                    Line::new(9, " new foo.bar.Baz(1, 2);", vec![5..(5 + 11)]),
                ],
            ),
        ];

        let matches = process_matched_file_fqcn(&fqcn, matches);
        assert_eq!(matches.len(), 2);

        assert_eq!("foo/bar/Baz.java", matches[0].file_path());
        assert_eq!(matches[0].lines().len(), 2);
        let mut lines = matches[0].lines();
        assert_equal(
            [(false, "package "), (true, "foo.bar")],
            lines.next().unwrap().iter().take(100),
        );
        assert_equal(
            [(false, "class "), (true, "Baz"), (false, " {};")],
            lines.next().unwrap().iter().take(100),
        );
        assert_eq!(None, lines.next());

        assert_eq!("foo/Quux.java", matches[1].file_path());
        assert_eq!(matches[1].lines().len(), 3);
        let mut lines = matches[1].lines();
        assert_equal(
            [(false, "import "), (true, "foo.bar.Baz")],
            lines.next().unwrap().iter().take(100),
        );
        assert_equal(
            [
                (false, "   "),
                (true, "Baz"),
                (false, " myBaz = new "),
                (true, "Baz"),
                (false, "()"),
            ],
            lines.next().unwrap().iter().take(100),
        );
        assert_equal(
            [(false, " new "), (true, "foo.bar.Baz"), (false, "(1, 2);")],
            lines.next().unwrap().iter().take(100),
        );
        assert_eq!(None, lines.next());
    }

    #[test]
    fn test_keeps_right_import() {
        let fqcn = Fqcn::new("foo.bar.Baz").unwrap();
        let matches = process_matched_file_fqcn(
            &fqcn,
            vec![MatchedFile::new(
                "foo/RightBaz.java",
                vec![Line::new(2, "import foo.bar.Baz;", vec![7..(7 + 11)])],
            )],
        );

        assert_eq!(1, matches.len());
        let matched_file = &matches[0];
        let mut lines = matched_file.lines();

        assert_equal(
            [(false, "import "), (true, "foo.bar.Baz"), (false, ";")],
            lines.next().unwrap().iter().take(100),
        );
        assert_eq!(None, lines.next());
    }

    #[test]
    fn test_filters_wrong_import() {
        let fqcn = Fqcn::new("foo.bar.Baz").unwrap();
        let matches = process_matched_file_fqcn(
            &fqcn,
            vec![MatchedFile::new(
                "foo/WrongBaz.java",
                vec![
                    Line::new(2, "import com.other.Baz;", vec![7..(7 + 13)]),
                    Line::new(8, "Baz b;", vec![0..3]),
                ],
            )],
        );

        assert_eq!(vec![] as Vec<MatchedFile>, matches);
    }
}
