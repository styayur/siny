// SPDX-License-Identifier: AGPL-3.0-only
use crate::Result;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Term {
    pub from: String,
    pub to: String,
}
pub fn validate(terms: &[Term]) -> Result<()> {
    if terms.len() > 256
        || terms
            .iter()
            .any(|t| t.from.is_empty() || t.from.len() > 256 || t.to.len() > 256)
    {
        return Err(
            "Terms require nonempty sources, at most 256 rules and 256 bytes per side".into(),
        );
    }
    Ok(())
}
/// Longest exact Unicode match, once per input position; replacements never cascade.
/// Explicit substring rules are shared by recognition and synthesis, not model tuning.
pub fn apply(text: &str, terms: &[Term]) -> String {
    let mut output = String::new();
    let mut tail = text;
    while !tail.is_empty() {
        if let Some(term) = terms
            .iter()
            .filter(|t| !t.from.is_empty() && tail.starts_with(&t.from))
            .max_by_key(|t| t.from.len())
        {
            output.push_str(&term.to);
            tail = &tail[term.from.len()..];
        } else {
            let c = tail.chars().next().expect("nonempty tail");
            output.push(c);
            tail = &tail[c.len_utf8()..];
        }
    }
    output
}
/// Character edit similarity after removing punctuation/case; diagnostic, not an accuracy guarantee.
pub fn similarity(expected: &str, actual: &str) -> f32 {
    let clean = |s: &str| {
        s.chars()
            .flat_map(char::to_lowercase)
            .filter(|c| c.is_alphanumeric())
            .collect::<Vec<_>>()
    };
    let a = clean(expected);
    let b = clean(actual);
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, left) in a.iter().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, right) in b.iter().enumerate() {
            let old = row[j + 1];
            row[j + 1] = (row[j] + 1)
                .min(old + 1)
                .min(previous + usize::from(left != right));
            previous = old;
        }
    }
    1.0 - row[b.len()] as f32 / a.len().max(b.len()) as f32
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacements_are_unicode_longest_and_non_cascading() {
        let rules = vec![
            Term {
                from: "语音".into(),
                to: "speech".into(),
            },
            Term {
                from: "speech".into(),
                to: "loop".into(),
            },
            Term {
                from: "语".into(),
                to: "short".into(),
            },
        ];
        assert_eq!(apply("语音 speech", &rules), "speech loop");
        assert!(validate(&[Term {
            from: String::new(),
            to: "x".into()
        }])
        .is_err());
    }
    #[test]
    fn verification_exposes_omissions() {
        assert_eq!(similarity("Hello, 世界!", "hello世界"), 1.0);
        assert!(similarity("every word must be read", "word read") < 0.8);
    }
}
