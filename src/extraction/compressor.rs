use rust_stemmers::{Algorithm, Stemmer};
use unicode_segmentation::UnicodeSegmentation;
use std::collections::HashMap;

/// Static array of common English stop-words (167 words, alphabetically sorted)
const STOP_WORDS: &[&str] = &[
    "a", "about", "above", "after", "again", "against", "all", "am", "an", "and", "another", "any", "anyone", "anything", "are", "arent", "as", "at",
    "be", "became", "because", "become", "becomes", "been", "before", "being", "below", "between", "both", "but", "by",
    "can", "cannot", "cant", "could", "couldnt", "did", "didnt", "do", "does", "doesnt", "doing", "dont", "down", "during",
    "each", "either", "else", "elsewhere", "enough", "etc", "even", "ever", "every", "everyone", "everything", "everywhere",
    "few", "for", "from", "further",
    "had", "hadnt", "has", "hasnt", "have", "havent", "having", "he", "her", "here", "hers", "herself", "him", "himself", "his", "how", "however",
    "i", "if", "in", "into", "is", "isnt", "it", "its", "itself",
    "just",
    "least", "less", "let", "lets", "like", "likely",
    "many", "me", "might", "more", "most", "must", "my", "myself",
    "no", "nor", "not", "nothing", "now",
    "of", "off", "on", "once", "only", "or", "other", "others", "ought", "our", "ours", "ourselves", "out", "over", "own",
    "same", "shall", "shant", "she", "should", "shouldnt", "since", "so", "some", "someone", "something", "somewhere", "such",
    "than", "that", "the", "their", "theirs", "them", "themselves", "then", "there", "these", "they", "this", "those", "through", "to", "too", "toward", "towards",
    "under", "until", "up", "upon", "us", "very", "via",
    "was", "wasnt", "we", "well", "were", "werent", "what", "whatever", "when", "whence", "whenever", "where", "whereafter", "whereas", "whereby", "wherein", "whereupon", "wherever", "whether", "which", "while", "whither", "who", "whoever", "whole", "whom", "whose", "why", "will", "with", "within", "without", "wont", "would", "wouldnt",
    "yes", "yet", "you", "your", "yours", "yourself", "yourselves"
];

/// A context compressor engine that uses TF-IDF and Porter Stemming to compress text.
pub struct ContextCompressor {
    stemmer: Stemmer,
}

impl ContextCompressor {
    /// Creates a new `ContextCompressor`.
    pub fn new() -> Self {
        Self {
            stemmer: Stemmer::create(Algorithm::English),
        }
    }

    /// Checks if a lowercased word is a stop word.
    fn is_stop_word(&self, word: &str) -> bool {
        STOP_WORDS.binary_search(&word).is_ok()
    }

    /// Cleans and stems a single word if it is not a stop word.
    fn stem_word(&self, word: &str) -> Option<String> {
        let clean = word
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_lowercase();

        if clean.is_empty() || self.is_stop_word(&clean) {
            None
        } else {
            Some(self.stemmer.stem(&clean).to_string())
        }
    }

    /// Scores all sentences in the text using TF-IDF.
    /// Returns a vector of tuples containing the sentence and its score in original order.
    pub fn score_sentences(&self, text: &str) -> Vec<(String, f64)> {
        // 1. Segment text into sentences
        let sentences: Vec<&str> = text
            .unicode_sentences()
            .filter(|s| !s.trim().is_empty())
            .collect();

        if sentences.is_empty() {
            return Vec::new();
        }

        // 2. Tokenize and stem each sentence to gather term frequencies (TF) and document frequencies (DF)
        let mut sentence_terms: Vec<HashMap<String, usize>> = Vec::new();
        let mut term_dfs: HashMap<String, usize> = HashMap::new();

        for &sentence in &sentences {
            let mut terms = HashMap::new();
            for word in sentence.unicode_words() {
                if let Some(stemmed) = self.stem_word(word) {
                    *terms.entry(stemmed).or_insert(0) += 1;
                }
            }

            // Update document frequency
            for term in terms.keys() {
                *term_dfs.entry(term.clone()).or_insert(0) += 1;
            }

            sentence_terms.push(terms);
        }

        // 3. Compute Inverse Document Frequency (IDF) for all unique terms
        let n = sentences.len() as f64;
        let mut term_idfs: HashMap<String, f64> = HashMap::new();
        for (term, &df) in &term_dfs {
            // Smooth IDF calculation: ln((1 + N) / (1 + DF)) + 1.0
            let idf = ((1.0 + n) / (1.0 + df as f64)).ln() + 1.0;
            term_idfs.insert(term.clone(), idf);
        }

        // 4. Score each sentence: sum of (TF * IDF) normalized by log of sentence word count
        let mut scored_sentences = Vec::with_capacity(sentences.len());
        for (idx, &sentence) in sentences.iter().enumerate() {
            let terms = &sentence_terms[idx];
            let mut sum_tfidf = 0.0;
            for (term, &tf) in terms {
                if let Some(&idf) = term_idfs.get(term) {
                    sum_tfidf += (tf as f64) * idf;
                }
            }

            let word_count = sentence.unicode_words().count();
            // Logarithmic length normalization to prevent bias towards extremely long sentences
            let score = if word_count > 0 {
                sum_tfidf / (1.0 + word_count as f64).ln()
            } else {
                0.0
            };

            scored_sentences.push((sentence.to_string(), score));
        }

        scored_sentences
    }

    /// Compresses the text by keeping a certain ratio of top-scoring sentences.
    /// Preserves original ordering. Ratio should be between 0.0 and 1.0.
    pub fn compress_by_ratio(&self, text: &str, ratio: f32) -> String {
        let ratio = ratio.clamp(0.0, 1.0);
        let scored = self.score_sentences(text);
        if scored.is_empty() {
            return String::new();
        }

        let target_count = (scored.len() as f32 * ratio).ceil() as usize;
        self.reconstruct(scored, target_count)
    }

    /// Compresses the text by keeping up to `max_sentences` top-scoring sentences.
    /// Preserves original ordering.
    pub fn compress_to_sentence_count(&self, text: &str, max_sentences: usize) -> String {
        let scored = self.score_sentences(text);
        self.reconstruct(scored, max_sentences)
    }

    /// Reconstructs the compressed text by taking the top `keep_count` sentences and sorting them
    /// by their original index before joining them.
    fn reconstruct(&self, scored: Vec<(String, f64)>, keep_count: usize) -> String {
        if scored.is_empty() || keep_count == 0 {
            return String::new();
        }

        // Tag with original index
        let mut indexed: Vec<(usize, String, f64)> = scored
            .into_iter()
            .enumerate()
            .map(|(idx, (s, score))| (idx, s, score))
            .collect();

        // Sort by score descending
        indexed.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        // Truncate to keep_count
        indexed.truncate(keep_count);

        // Sort back to original document order
        indexed.sort_by_key(|a| a.0);

        // Join sentences
        let sentences: Vec<String> = indexed.into_iter().map(|(_, s, _)| s.trim().to_string()).collect();
        sentences.join(" ")
    }
}

impl Default for ContextCompressor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stop_words_list() {
        let compressor = ContextCompressor::new();
        assert!(compressor.is_stop_word("the"));
        assert!(compressor.is_stop_word("about"));
        assert!(compressor.is_stop_word("wouldnt"));
        assert!(!compressor.is_stop_word("rust"));
        assert!(!compressor.is_stop_word("compression"));
    }

    #[test]
    fn test_stemming() {
        let compressor = ContextCompressor::new();
        assert_eq!(compressor.stem_word("connecting"), Some("connect".to_string()));
        assert_eq!(compressor.stem_word("connections"), Some("connect".to_string()));
        // Stop word should be filtered out
        assert_eq!(compressor.stem_word("the"), None);
        // Non-alphanumeric cleanup
        assert_eq!(compressor.stem_word("compilers!"), Some("compil".to_string()));
    }

    #[test]
    fn test_score_sentences() {
        let compressor = ContextCompressor::new();
        let text = "Rust is a programming language. It is extremely fast and safe. Fluff sentence here. Another fluff sentence.";
        let scored = compressor.score_sentences(text);

        assert_eq!(scored.len(), 4);
        // The sentences with unique, rich terms should rank higher than generic fluff sentences
        // Let's assert the relative order or check scores
        for (sentence, score) in &scored {
            println!("Sentence: '{}' -> Score: {}", sentence, score);
        }

        // Sentence 0 and 1 have more informative words (programming, language, extremely, fast, safe)
        // sentence 2 and 3 have (fluff, sentence, another) which are highly repetitive or stop-words
        let score_lang = scored[0].1;
        let score_fast = scored[1].1;
        let score_fluff1 = scored[2].1;
        let score_fluff2 = scored[3].1;

        assert!(score_lang > 0.0);
        assert!(score_fast > 0.0);
        assert!(score_lang > score_fluff1);
        assert!(score_fast > score_fluff2);
    }

    #[test]
    fn test_compress_by_ratio() {
        let compressor = ContextCompressor::new();
        let text = "First important point. Second generic fluff. Third highly critical information. Fourth random remark.";
        
        // Ratio of 0.5 should keep 2 sentences
        let compressed = compressor.compress_by_ratio(text, 0.5);
        let sentences: Vec<&str> = compressed.unicode_sentences().collect();
        assert_eq!(sentences.len(), 2);
    }

    #[test]
    fn test_compress_to_sentence_count() {
        let compressor = ContextCompressor::new();
        let text = "This is a statement about databases. Databases store persistent data. I like apples. Apples are round fruit. Database queries are executed.";

        // We want to compress to 3 sentences. It should pick the database related ones if we have "database" repeated multiple times (making it high DF but also specific terms like persistent, queries, executed).
        // Let's check that the output has exactly 3 sentences.
        let compressed = compressor.compress_to_sentence_count(text, 3);
        let sentences: Vec<&str> = compressed.unicode_sentences().collect();
        assert_eq!(sentences.len(), 3);
    }

    #[test]
    fn test_preserves_original_order() {
        let compressor = ContextCompressor::new();
        let text = "First sentence. Second sentence. Third sentence. Fourth sentence.";
        let compressed = compressor.compress_to_sentence_count(text, 2);
        
        // Let's find index of sentences in compressed text
        let pos1 = compressed.find("First");
        let pos2 = compressed.find("Second");
        let pos3 = compressed.find("Third");
        let pos4 = compressed.find("Fourth");

        // Verify that whatever sentences are chosen, they remain in their original order.
        let mut positions = vec![pos1, pos2, pos3, pos4];
        positions.retain(|p| p.is_some());
        
        for i in 0..positions.len() - 1 {
            assert!(positions[i].unwrap() < positions[i + 1].unwrap());
        }
    }
}
