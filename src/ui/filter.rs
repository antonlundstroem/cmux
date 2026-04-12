use nucleo_matcher::{
    pattern::{CaseMatching, Normalization, Pattern},
    Matcher,
};

pub struct Filter {
    matcher: Matcher,
}

impl Filter {
    pub fn new() -> Self {
        Self {
            matcher: Matcher::new(nucleo_matcher::Config::DEFAULT),
        }
    }

    /// Rank `items` against `query`. Items with no match are dropped. Returns
    /// indices in descending score order, with ties broken by original order.
    /// An empty query keeps all items in their original order.
    pub fn rank<'a, I>(&mut self, query: &str, items: I) -> Vec<usize>
    where
        I: IntoIterator<Item = &'a str>,
    {
        if query.is_empty() {
            return items.into_iter().enumerate().map(|(i, _)| i).collect();
        }
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        let mut buf = Vec::new();
        let mut scored: Vec<(usize, u32)> = items
            .into_iter()
            .enumerate()
            .filter_map(|(i, s)| {
                buf.clear();
                let haystack = nucleo_matcher::Utf32Str::new(s, &mut buf);
                pattern
                    .score(haystack, &mut self.matcher)
                    .map(|score| (i, score))
            })
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        scored.into_iter().map(|(i, _)| i).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_keeps_all() {
        let mut f = Filter::new();
        let items = vec!["alpha", "beta", "gamma"];
        let out = f.rank("", items.iter().copied());
        assert_eq!(out, vec![0, 1, 2]);
    }

    #[test]
    fn fuzzy_filters_and_ranks() {
        let mut f = Filter::new();
        let items = vec!["authentication", "buffer", "authorization", "debug"];
        let out = f.rank("auth", items.iter().copied());
        let matched: Vec<&str> = out.iter().map(|i| items[*i]).collect();
        assert!(matched.contains(&"authentication"));
        assert!(matched.contains(&"authorization"));
        assert!(!matched.contains(&"buffer"));
    }
}
