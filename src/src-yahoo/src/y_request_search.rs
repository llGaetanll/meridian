use crate::YRequest;

/// Minimal percent-encoding for a query value.
fn urlencoding(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            c => c
                .to_string()
                .as_bytes()
                .iter()
                .map(|b| format!("%{b:02X}"))
                .collect(),
        })
        .collect()
}

pub struct YRequestSearch<'a> {
    query: &'a str,
}

impl<'a> YRequestSearch<'a> {
    /// Create a new ticker search request
    pub fn new(query: &'a str) -> Self {
        YRequestSearch { query }
    }
}

impl<'a> YRequest for YRequestSearch<'a> {
    fn path(&self) -> String {
        format!("v1/finance/search?q={}", urlencoding(self.query))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_the_query() {
        let req = YRequestSearch::new("s&p 500");

        assert_eq!(req.path(), "v1/finance/search?q=s%26p%20500");
    }
}
