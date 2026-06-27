#![allow(dead_code)]
use regex::Regex;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedFact {
    pub from: String,
    pub relation: String,
    pub to: String,
}

pub struct FactExtractor {
    relation_patterns: Vec<(Regex, &'static str)>,
}

const STOP_WORDS: &[&str] = &[
    "a", "about", "above", "after", "again", "against", "all", "am", "an", "and", "any", "anyone", "anything", "are", "as", "at",
    "be", "because", "been", "before", "being", "below", "between", "both", "but", "by",
    "can", "did", "do", "does", "doing", "don", "down", "during",
    "each",
    "few", "for", "from", "further",
    "had", "has", "have", "having", "he", "her", "here", "hers", "herself", "him", "himself", "his", "how",
    "i", "if", "in", "into", "is", "it", "its", "itself",
    "just",
    "me", "more", "most", "my", "myself",
    "no", "nor", "not", "now",
    "of", "off", "on", "once", "only", "or", "other", "our", "ours", "ourselves", "out", "over", "own",
    "s", "same", "she", "should", "so", "some", "someone", "something", "t", "than", "that", "the", "their", "theirs", "them", "themselves", "then", "there", "these", "they", "this", "those", "through", "to", "too",
    "under", "until", "up",
    "very",
    "was", "we", "were", "what", "when", "where", "which", "who", "whom", "why", "will", "with",
    "you", "your", "yours", "yourself", "yourselves",
];

const COMMON_NOUNS: &[&str] = &[
    "app", "application", "code", "compiler", "computer", "database", "db", "developer", "engine", "engineer", "file", "framework", "hardware", "interpreter", "job", "language", "library", "machine", "program", "programmer", "project", "server", "software", "system", "thing", "things", "tool", "user", "work",
];

fn is_valid_entity(word: &str) -> bool {
    let lower = word.to_lowercase();
    if lower.is_empty() {
        return false;
    }
    if STOP_WORDS.binary_search(&lower.as_str()).is_ok() {
        return false;
    }
    if word == lower && COMMON_NOUNS.binary_search(&lower.as_str()).is_ok() {
        return false;
    }
    true
}

impl FactExtractor {
    pub fn new() -> Self {
        Self {
            relation_patterns: vec![
                (Regex::new(r"(\w+)\s+(?:(?:is|are|was|were|am)\s+)?(?:uses?|using)\s+(\w+)").unwrap(), "uses"),
                (Regex::new(r"(\w+)\s+(?:depends?\s+on|requires?)\s+(\w+)").unwrap(), "depends_on"),
                (Regex::new(r"(\w+)\s+(?:prefers?|likes?|favou?rs?)\s+(\w+)").unwrap(), "prefers"),
                (Regex::new(r"(\w+)\s+(?:is\s+a|is\s+an|is\s+the)\s+(\w+)").unwrap(), "is_a"),
                (Regex::new(r"(\w+)\s+(?:(?:is|are|was|were|am)\s+)?(?:works?|working)\s+(?:on|with|at)\s+(\w+)").unwrap(), "works_with"),
                (Regex::new(r"(\w+)\s+(?:created?|built?|wrote?)\s+(\w+)").unwrap(), "created"),
            ],
        }
    }

    pub fn extract(&self, text: &str) -> Vec<ExtractedFact> {
        let mut facts = Vec::new();
        for sentence in text.unicode_sentences() {
            for (pattern, relation_type) in &self.relation_patterns {
                for caps in pattern.captures_iter(sentence) {
                    let from = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                    let to = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                    if is_valid_entity(from) && is_valid_entity(to) {
                        facts.push(ExtractedFact {
                            from: from.to_string(),
                            relation: relation_type.to_string(),
                            to: to.to_string(),
                        });
                    }
                }
            }
        }
        facts
    }
}

impl Default for FactExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fact_extraction() {
        let extractor = FactExtractor::new();

        // Test uses/using
        let text1 = "Alice uses Rust. Bob is using Python.";
        let facts1 = extractor.extract(text1);
        assert_eq!(facts1.len(), 2);
        assert_eq!(facts1[0], ExtractedFact {
            from: "Alice".to_string(),
            relation: "uses".to_string(),
            to: "Rust".to_string(),
        });
        assert_eq!(facts1[1], ExtractedFact {
            from: "Bob".to_string(),
            relation: "uses".to_string(),
            to: "Python".to_string(),
        });

        // Test depends_on/requires
        let text2 = "openmemory_rs depends on SQLite. This engine requires tokio.";
        let facts2 = extractor.extract(text2);
        // Note: "engine" (from "This engine") is ignored because it's a lowercase common noun.
        assert_eq!(facts2.len(), 1);
        assert_eq!(facts2[0], ExtractedFact {
            from: "openmemory_rs".to_string(),
            relation: "depends_on".to_string(),
            to: "SQLite".to_string(),
        });

        // Test prefers/likes/favours
        let text3 = "Charlie prefers Neovim. Dave likes VSCode. Eve favours Emacs.";
        let facts3 = extractor.extract(text3);
        assert_eq!(facts3.len(), 3);
        assert_eq!(facts3[0], ExtractedFact {
            from: "Charlie".to_string(),
            relation: "prefers".to_string(),
            to: "Neovim".to_string(),
        });
        assert_eq!(facts3[1], ExtractedFact {
            from: "Dave".to_string(),
            relation: "prefers".to_string(),
            to: "VSCode".to_string(),
        });
        assert_eq!(facts3[2], ExtractedFact {
            from: "Eve".to_string(),
            relation: "prefers".to_string(),
            to: "Emacs".to_string(),
        });

        // Test is_a
        let text4 = "Rust is a language. Python is an interpreter. sqlite is the database.";
        let facts4 = extractor.extract(text4);
        // Note: "language" and "interpreter" are common nouns (so "Rust is a language" is ignored since "language" is lowercase common noun).
        // Let's verify what remains. None should remain because language, interpreter, database are all lowercase common nouns.
        assert_eq!(facts4.len(), 0);

        // Let's test custom capitalized names with is_a
        let text4_custom = "Alice is a Developer. Python is an Interpreter.";
        let facts4_custom = extractor.extract(text4_custom);
        // "Developer" and "Interpreter" are capitalized, so they are kept.
        assert_eq!(facts4_custom.len(), 2);
        assert!(facts4_custom.contains(&ExtractedFact {
            from: "Alice".to_string(),
            relation: "is_a".to_string(),
            to: "Developer".to_string(),
        }));

        // Test works_with/works_on/works_at
        let text5 = "Alice works with Bob. Charlie works on compiler. Dave works at Google.";
        let facts5 = extractor.extract(text5);
        // "compiler" is lowercase common noun, so it is ignored.
        // Alice works with Bob -> kept
        // Dave works at Google -> kept
        assert_eq!(facts5.len(), 2);
        assert!(facts5.contains(&ExtractedFact {
            from: "Alice".to_string(),
            relation: "works_with".to_string(),
            to: "Bob".to_string(),
        }));
        assert!(facts5.contains(&ExtractedFact {
            from: "Dave".to_string(),
            relation: "works_with".to_string(),
            to: "Google".to_string(),
        }));

        // Test created/built/wrote
        let text6 = "Satoshi created Bitcoin. Torvalds built Linux. Shakespeare wrote Hamlet.";
        let facts6 = extractor.extract(text6);
        assert_eq!(facts6.len(), 3);
        assert_eq!(facts6[0], ExtractedFact {
            from: "Satoshi".to_string(),
            relation: "created".to_string(),
            to: "Bitcoin".to_string(),
        });
        assert_eq!(facts6[1], ExtractedFact {
            from: "Torvalds".to_string(),
            relation: "created".to_string(),
            to: "Linux".to_string(),
        });
        assert_eq!(facts6[2], ExtractedFact {
            from: "Shakespeare".to_string(),
            relation: "created".to_string(),
            to: "Hamlet".to_string(),
        });

        // Test stop word filtering
        let text7 = "He prefers Rust. She prefers Python. It uses postgres.";
        let facts7 = extractor.extract(text7);
        assert_eq!(facts7.len(), 0);
    }
}
