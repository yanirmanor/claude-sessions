use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::session::Session;

pub fn fuzzy_filter(sessions: &[Session], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..sessions.len()).collect();
    }

    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    let pattern = Pattern::new(
        query,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );

    let mut scored: Vec<(usize, u32)> = Vec::new();

    for (i, session) in sessions.iter().enumerate() {
        let haystack = format!(
            "{} {} {}",
            session.first_user_message,
            session.git_branch.as_deref().unwrap_or(""),
            session.id
        );
        let mut buf = Vec::new();
        let haystack_utf32 = Utf32Str::new(&haystack, &mut buf);
        if let Some(score) = pattern.score(haystack_utf32, &mut matcher) {
            scored.push((i, score));
        }
    }

    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.into_iter().map(|(i, _)| i).collect()
}
