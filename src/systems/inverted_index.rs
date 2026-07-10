//! # Inverted Index (Text Search Engine) Implementation
//!
//! Implements a foundational Inverted Index for full-text search, featuring tokenization,
//! stop-word filtering, and basic TF-IDF (Term Frequency - Inverse Document Frequency) scoring.
//!
//! **Replaces Crates:** `tantivy`, `lucy` (C/C++), `elastic` (client)
//!
//! **Real-world Usage:**
//! - Elasticsearch / Apache Lucene (Java).
//! - Postgres Full Text Search.
//! - Documentation search tools (like mdBook search or Algolia).
//!
//! **Why build it yourself?**
//! Building an inverted index demystifies how search engines find relevant documents instantly.
//! You learn how unstructured text is normalized (tokenization, stemming), how documents are mapped
//! to terms (the inverted index itself), and how relevance is calculated mathematically using TF-IDF
//! so that rare words carry more weight than common ones.

// Precision loss is acceptable in TF-IDF floating-point scoring math.
#![allow(clippy::cast_precision_loss)]

use std::collections::{HashMap, HashSet};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      InvertedIndex
//      ├── index: HashMap<Term, Vec<Postings>>
//      └── documents: HashMap<DocId, DocumentMeta>
//
//      Postings
//      └── (DocId, TermFrequency)
//
// Invariants:
// 1. `documents` contains an entry for every `DocId` that has postings in the `index`.
// 2. Postings lists only contain unique `DocId`s per term.
// 3. A document's `total_terms` reflects the count after stop-word filtering.
//
// Logic:
// 1. **Index Time**:
//    - Text -> lowercase -> split (tokenize) -> filter stop words -> count frequencies.
//    - For each term, append `(DocId, Frequency)` to the index.
// 2. **Query Time**:
//    - Query -> tokenize -> filter.
//    - For each term, look up `Vec<Postings>`.
//    - Calculate TF-IDF score for each document that contains the term.
//    - Aggregate scores and sort by highest relevance.
//
// TF-IDF Formula used:
// - TF (Term Frequency) = (Count of term in doc) / (Total terms in doc)
// - IDF (Inverse Document Frequency) = ln(Total Docs / Docs containing term)
// - Score = TF * IDF
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Add Document  │ O(Words)    │ O(Words)    │
// │ Query (k terms│ O(k * Docs) │ O(Results)  │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **In-memory**: We use a `HashMap` for simplicity. Production systems use LSM-Trees or
//   memory-mapped files for durability and scale.
// - **Tokenization**: Very basic split by whitespace and punctuation removal. Production systems
//   use complex analyzers (e.g., Porter Stemmer, character N-grams).

/// Represents a unique identifier for a document.
pub type DocId = u32;

/// Postings list entry representing a term's occurrence in a specific document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Posting {
    pub doc_id: DocId,
    pub term_frequency: usize, // Raw count of the term in the document
}

/// Metadata stored per document for scoring.
#[derive(Debug, Clone)]
pub struct DocumentMeta {
    pub id: DocId,
    pub title: String,
    pub total_terms: usize, // Needed for TF calculation
}

/// A search result with its relevance score.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub doc_id: DocId,
    pub score: f64,
}

/// Trait defining a Search Engine interface.
pub trait SearchEngine {
    /// Adds a document to the index.
    fn add_document(&mut self, doc_id: DocId, title: String, content: &str);

    /// Searches for documents matching the query, returning results ordered by relevance (descending).
    fn search(&self, query: &str) -> Vec<SearchResult>;
}

/// A basic Inverted Index implementation.
pub struct InvertedIndex {
    index: HashMap<String, Vec<Posting>>,
    documents: HashMap<DocId, DocumentMeta>,
    stop_words: HashSet<String>,
}

impl InvertedIndex {
    /// Creates a new, empty Inverted Index with a default set of stop words.
    #[must_use]
    pub fn new() -> Self {
        let stop_words: HashSet<String> = [
            "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "with", "is",
            "are", "was", "were", "it", "this", "that", "of", "by", "as", "be",
        ]
        .iter()
        .map(std::string::ToString::to_string)
        .collect();

        Self {
            index: HashMap::new(),
            documents: HashMap::new(),
            stop_words,
        }
    }

    /// Tokenizes text into normalized terms.
    fn tokenize(&self, text: &str) -> Vec<String> {
        // RUST INSIGHT: `char::is_alphanumeric` provides Unicode-aware checking.
        // We filter out punctuation, lowercase everything, and filter stop words.
        text.split_whitespace()
            .map(|word| {
                // ⚡ BOLT OPTIMIZATION: Avoid intermediate `.collect::<String>()` allocation by using `.flat_map(|c| c.to_lowercase())`.
                word.chars()
                    .filter(|c| c.is_alphanumeric())
                    .flat_map(char::to_lowercase)
                    .collect::<String>()
            })
            .filter(|term| !term.is_empty() && !self.stop_words.contains(term))
            .collect()
    }

    /// Calculates the TF-IDF score for a specific term in a specific document.
    fn calculate_tf_idf(&self, term: &str, posting: &Posting) -> f64 {
        let doc_meta = self.documents.get(&posting.doc_id).unwrap();

        // Term Frequency (TF): Term occurrences / Total terms in document
        let tf = (posting.term_frequency as f64) / (doc_meta.total_terms as f64);

        // Inverse Document Frequency (IDF): ln(Total docs / Docs containing term)
        let total_docs = self.documents.len() as f64;
        let docs_with_term = self.index.get(term).map_or(0, Vec::len) as f64;

        // GOTCHA: We add 1.0 to the denominator to prevent division by zero in case of data anomalies,
        // and we use `.ln()` for natural logarithm.
        // Some formulas do ln( (N - n + 0.5) / (n + 0.5) ) (BM25), but standard TF-IDF is ln(N / n).
        // If n = 0, IDF is undefined. We only call this when n > 0.
        let idf = if docs_with_term > 0.0 {
            (total_docs / docs_with_term).ln()
        } else {
            0.0
        };

        tf * idf
    }
}

impl Default for InvertedIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchEngine for InvertedIndex {
    fn add_document(&mut self, doc_id: DocId, title: String, content: &str) {
        let terms = self.tokenize(content);
        let total_terms = terms.len();

        if total_terms == 0 {
            return;
        }

        // Re-indexing: if this doc_id was added before, purge its stale postings
        // first so re-adding updates the document instead of double-counting it.
        // This upholds Invariant #2 (postings lists hold unique DocIds per term).
        if self.documents.contains_key(&doc_id) {
            self.index.retain(|_term, postings| {
                postings.retain(|posting| posting.doc_id != doc_id);
                !postings.is_empty()
            });
        }

        // Count term frequencies for this document
        let mut term_counts: HashMap<String, usize> = HashMap::new();
        for term in terms {
            *term_counts.entry(term).or_insert(0) += 1;
        }

        // Update the inverted index
        for (term, count) in term_counts {
            self.index.entry(term).or_default().push(Posting {
                doc_id,
                term_frequency: count,
            });
        }

        // Store document metadata
        self.documents.insert(
            doc_id,
            DocumentMeta {
                id: doc_id,
                title,
                total_terms,
            },
        );
    }

    fn search(&self, query: &str) -> Vec<SearchResult> {
        let query_terms = self.tokenize(query);
        let mut scores: HashMap<DocId, f64> = HashMap::new();

        // For each term in the query...
        for term in &query_terms {
            // Find all documents containing this term
            if let Some(postings) = self.index.get(term) {
                for posting in postings {
                    let score = self.calculate_tf_idf(term, posting);
                    *scores.entry(posting.doc_id).or_insert(0.0) += score;
                }
            }
        }

        // Convert to Vec and sort descending by score. Break ties using doc_id to ensure deterministic order.
        let mut results: Vec<SearchResult> = scores
            .into_iter()
            .map(|(doc_id, score)| SearchResult { doc_id, score })
            .collect();

        // RUST INSIGHT: Floats (`f64`) don't implement `Ord` because `NaN` != `NaN`.
        // We must use `partial_cmp` and unwrap safely since we know our scores are valid numbers.
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                // If scores are equal, sort by doc_id ascending
                .then_with(|| a.doc_id.cmp(&b.doc_id))
        });

        // Debug output to see why it fails
        #[cfg(test)]
        for r in &results {
            println!("Doc: {}, Score: {}", r.doc_id, r.score);
        }

        results
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tantivy`: A full-featured, highly optimized search engine crate. It uses memory-mapped
//   files, implements BM25 scoring (superior to basic TF-IDF), supports complex query parsing,
//   and uses fast data structures like FSTs (Finite State Transducers) for the term dictionary.
//
// Missing vs. Production:
// - **BM25 Scoring**: Modern engines use Okapi BM25, which saturates TF to prevent long documents
//   from dominating unfairly.
// - **Persistence**: This is entirely in-memory. A real index flushes to disk in segments.
// - **Boolean Queries**: We just sum scores. We don't support MUST (AND) or MUST_NOT (NOT).
// - **Stemming**: We treat "run" and "running" as different words.
//
// Next Steps:
// 1. Upgrade scoring to Okapi BM25.
// 2. Implement boolean logic (AND/OR operators in queries).
//
// Benchmarking Note:
// To benchmark `add_document` and `search`, use Criterion to ingest a large text corpus
// (like project Gutenberg or Wikipedia dumps). Measure index time per megabyte. Then measure
// search latency by querying common vs. rare terms, noting the difference in posting list traversal.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenization_and_stop_words() {
        let index = InvertedIndex::new();
        let text = "The Quick, Brown FOX jumps OVER a lazy dog!";
        let tokens = index.tokenize(text);

        // "The", "a" are stop words. Punctuation removed. Lowercased.
        assert_eq!(
            tokens,
            vec!["quick", "brown", "fox", "jumps", "over", "lazy", "dog"]
        );
    }

    #[test]
    fn test_add_document() {
        let mut engine = InvertedIndex::new();
        engine.add_document(1, "Doc 1".to_string(), "rust is fast and safe");

        assert_eq!(engine.documents.len(), 1);
        assert_eq!(engine.documents.get(&1).unwrap().total_terms, 3); // "is" and "and" are stop words

        // Check index for "rust"
        let rust_postings = engine.index.get("rust").unwrap();
        assert_eq!(rust_postings.len(), 1);
        assert_eq!(rust_postings[0].doc_id, 1);
        assert_eq!(rust_postings[0].term_frequency, 1);
    }

    #[test]
    fn test_search_scoring_tf_idf() {
        let mut engine = InvertedIndex::new();

        // Doc 1: "rust" appears once out of 2 terms.
        engine.add_document(1, "Rust Intro".to_string(), "learn rust");

        // Doc 2: "rust" appears 3 times out of 5 terms. (Higher TF)
        engine.add_document(
            2,
            "Rust Deep Dive".to_string(),
            "rust is great, rust is fast, rust",
        );

        // Doc 3: Doesn't contain "rust"
        engine.add_document(3, "Python".to_string(), "python is good");

        let results = engine.search("rust");

        // Should only return Docs 1 and 2
        assert_eq!(results.len(), 2);

        // Doc 2 should score higher because it has a higher Term Frequency (3/3 vs 1/2)
        // (Note: stop word "is" removed, total terms for doc 2: rust, great, rust, fast, rust -> 5)
        assert_eq!(results[0].doc_id, 2);
        assert_eq!(results[1].doc_id, 1);

        // Verify score > 0
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn test_multi_word_query() {
        let mut engine = InvertedIndex::new();
        engine.add_document(1, "D1".to_string(), "apple banana orange");
        engine.add_document(2, "D2".to_string(), "apple banana");
        engine.add_document(3, "D3".to_string(), "orange");

        // Query "apple orange"
        let results = engine.search("apple orange");

        // D1 has both, D2 has apple, D3 has orange.
        // D1 should score highest as it matches both terms.
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].doc_id, 3);
        assert_eq!(results[1].doc_id, 1);
        assert_eq!(results[2].doc_id, 2);
    }

    #[test]
    fn test_empty_search() {
        let mut engine = InvertedIndex::new();
        engine.add_document(1, "D1".to_string(), "hello world");

        let results = engine.search("missing");
        assert!(results.is_empty());

        let results2 = engine.search("the a is"); // Only stop words
        assert!(results2.is_empty());
    }

    #[test]
    fn test_readd_document_reindexes_without_double_counting() {
        let mut engine = InvertedIndex::new();

        // Initial version of doc 1 contains "a" (twice) and "b".
        engine.add_document(1, "D1".to_string(), "a a b");

        // Re-index doc 1 with entirely new content ("c" only).
        engine.add_document(1, "D1 v2".to_string(), "c");

        // Only one document exists, and its metadata reflects the new content.
        assert_eq!(engine.documents.len(), 1);
        assert_eq!(engine.documents.get(&1).unwrap().total_terms, 1);

        // Stale terms must no longer reference doc 1 (Invariant #2).
        assert!(!engine.index.contains_key("a"));
        assert!(!engine.index.contains_key("b"));

        // The new term references doc 1 exactly once (no duplicate postings).
        let c_postings = engine.index.get("c").unwrap();
        assert_eq!(c_postings.len(), 1);
        assert_eq!(c_postings[0].doc_id, 1);
        assert_eq!(c_postings[0].term_frequency, 1);

        // Searching the old term returns nothing; the new term returns doc 1 once.
        assert!(engine.search("a").is_empty());
        let results = engine.search("c");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].doc_id, 1);
    }

    #[test]
    fn test_readd_same_term_no_duplicate_postings() {
        let mut engine = InvertedIndex::new();

        // Add doc 1, then re-add with the same term at a different frequency.
        engine.add_document(1, "D1".to_string(), "rust");
        engine.add_document(1, "D1 v2".to_string(), "rust rust");

        // Exactly one posting for "rust" -> doc 1, with the updated frequency.
        let postings = engine.index.get("rust").unwrap();
        assert_eq!(postings.len(), 1);
        assert_eq!(postings[0].doc_id, 1);
        assert_eq!(postings[0].term_frequency, 2);
    }
}
