//! Bounded, cached lexical retrieval. No embedding model or network required.
use crate::normalize;
use std::collections::{BTreeMap, BTreeSet};

pub fn query_terms(text: &str) -> Vec<String> {
    normalize(text)
        .split_whitespace()
        .filter(|w| {
            w.len() > 2
                && !matches!(
                    *w,
                    "what"
                        | "when"
                        | "where"
                        | "which"
                        | "this"
                        | "that"
                        | "with"
                        | "from"
                        | "have"
                        | "about"
                        | "tell"
                        | "please"
                        | "could"
                        | "would"
                        | "the"
                        | "and"
                        | "for"
                        | "how"
                        | "why"
                        | "does"
                        | "are"
                        | "can"
                        | "you"
                        | "explain"
                        | "more"
                        | "said"
                        | "says"
                )
        })
        .map(str::to_owned)
        .take(256)
        .collect()
}
struct Chunk {
    document: usize,
    text: String,
    frequencies: BTreeMap<String, usize>,
    length: usize,
}
#[derive(Default)]
pub struct RetrievalIndex {
    chunks: Vec<Chunk>,
    postings: BTreeMap<String, Vec<usize>>,
    average_length: f64,
}
pub struct RetrievalHit<'a> {
    pub document: usize,
    pub text: &'a str,
    pub score: f64,
}
impl RetrievalIndex {
    pub fn build<'a>(documents: impl IntoIterator<Item = (usize, &'a str)>) -> Self {
        let mut index = Self::default();
        for (document, text) in documents.into_iter().take(200) {
            // Inputs are already bounded by transcript validation. Keep at most
            // 128 excerpts per document and 800 Unicode characters per excerpt.
            for text in excerpts(text).into_iter().take(128) {
                let mut frequencies = BTreeMap::new();
                let words = query_terms(&text);
                let length = words.len();
                for term in words {
                    *frequencies.entry(term).or_insert(0) += 1;
                }
                if length < 2 {
                    continue;
                }
                let id = index.chunks.len();
                for term in frequencies.keys() {
                    index.postings.entry(term.clone()).or_default().push(id);
                }
                index.chunks.push(Chunk {
                    document,
                    text,
                    frequencies,
                    length,
                });
            }
        }
        index.average_length = index.chunks.iter().map(|c| c.length).sum::<usize>() as f64
            / index.chunks.len().max(1) as f64;
        index
    }
    pub fn search(&self, query: &str, limit: usize) -> Vec<RetrievalHit<'_>> {
        let terms: BTreeSet<_> = query_terms(query).into_iter().take(64).collect();
        if terms.len() < 2 {
            return Vec::new();
        }
        let mut candidates: BTreeMap<usize, (usize, f64)> = BTreeMap::new();
        for term in &terms {
            let Some(postings) = self.postings.get(term) else {
                continue;
            };
            let n = self.chunks.len() as f64;
            let df = postings.len() as f64;
            let idf = (1.0 + (n - df + 0.5) / (df + 0.5)).ln();
            for &id in postings {
                let c = &self.chunks[id];
                let tf = c.frequencies[term] as f64;
                let score = idf * tf * 2.2
                    / (tf + 1.2 * (0.25 + 0.75 * c.length as f64 / self.average_length.max(1.0)));
                let hit = candidates.entry(id).or_default();
                hit.0 += 1;
                hit.1 += score;
            }
        }
        let mut scored: Vec<_> = candidates
            .into_iter()
            .filter(|(_, (overlap, _))| *overlap >= 2 && overlap * 2 >= terms.len())
            .collect();
        scored.sort_by(|a, b| b.1 .1.total_cmp(&a.1 .1).then(a.0.cmp(&b.0)));
        // One excerpt per source document prevents a long source monopolizing context.
        let mut seen = BTreeSet::new();
        scored
            .into_iter()
            .filter_map(|(id, (_, score))| {
                let c = &self.chunks[id];
                seen.insert(c.document).then_some(RetrievalHit {
                    document: c.document,
                    text: &c.text,
                    score,
                })
            })
            .take(limit.min(4))
            .collect()
    }
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
}

fn excerpts(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    for sentence in text
        .split(['\n', '.', '!', '?'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let chars: Vec<_> = sentence.chars().collect();
        let mut start = 0;
        while start < chars.len() && result.len() < 128 {
            let end = (start + 800).min(chars.len());
            result.push(chars[start..end].iter().collect());
            if end == chars.len() {
                break;
            }
            start = end - 100;
        }
        if result.len() == 128 {
            break;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranks_sources_and_abstains_without_coverage() {
        let index = RetrievalIndex::build([(0,"Solar panels convert sunlight into electricity."),(1,"Solar panels have a warranty. Solar panels convert sunlight into electricity efficiently.")]);
        let hits = index.search("How do solar panels convert sunlight?", 4);
        assert_eq!(hits.len(), 2);
        assert!(hits[0].text.contains("sunlight"));
        assert!(index
            .search("solar unrelated elephant astronomy", 4)
            .is_empty());
        assert!(index.search("solar", 4).is_empty());
    }
    #[test]
    fn bounded_unicode_and_deterministic_ties() {
        let text = "solar panels ".repeat(1000);
        let index = RetrievalIndex::build([(0, text.as_str()), (1, text.as_str())]);
        let hits = index.search("solar panels", 99);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].document, 0);
        assert!(hits[0].text.chars().count() <= 800);
    }
    #[test]
    fn long_paragraphs_keep_later_evidence() {
        let text = format!(
            "{} Solar panels convert sunlight",
            "ordinary words ".repeat(100)
        );
        let index = RetrievalIndex::build([(0, text.as_str())]);
        assert!(index.search("solar panels sunlight", 1)[0]
            .text
            .contains("sunlight"));
    }
}
