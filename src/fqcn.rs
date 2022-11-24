use std::ops::Range;

use regex::Regex;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Fqcn {
    value: String,
    package_range: Range<usize>,
    ident_range: Range<usize>,
}

impl Fqcn {
    pub fn new<S: Into<String>>(value: S) -> Option<Self> {
        let value = value.into();

        let re = Regex::new(r"^(([a-z0-9][a-z0-9\.]+)+\.)?([A-Z][\w]*)?$").unwrap();
        let captures = re.captures(value.as_ref())?;
        let package_range = captures.get(2).map(|m| m.range())?;
        let ident_range = captures.get(3).map(|m| m.range())?;

        Some(Fqcn {
            value,
            package_range,
            ident_range,
        })
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn package(&self) -> &str {
        &self.value[self.package_range.clone()]
    }

    pub fn package_with_trailing(&self) -> &str {
        &self.value[self.package_range.start..(self.package_range.end + 1)]
    }

    pub fn ident(&self) -> &str {
        &self.value[self.ident_range.clone()]
    }
}

#[cfg(test)]
mod test {
    use matches::assert_matches;

    use super::Fqcn;

    #[test]
    fn test_works() {
        let fqcn = Fqcn::new("foo.bar.Baz").unwrap();
        assert_eq!("Baz", fqcn.ident());
        assert_eq!("foo.bar", fqcn.package());
        assert_eq!("foo.bar.Baz", fqcn.value());

        assert!(Fqcn::new("foo.bar").is_none());
        assert_matches!(Fqcn::new("foo.bar.Baz.Smaz"), None);
    }
}
