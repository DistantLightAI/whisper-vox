/// Sentence boundary detection: splits on terminal punctuation (.?!)
/// while handling abbreviations and ellipsis.
pub struct SentenceDetector {
    buffer: String,
}

const ABBREVIATIONS: &[&str] = &[
    "Mr.", "Mrs.", "Ms.", "Dr.", "Prof.", "Sr.", "Jr.", "St.",
    "vs.", "etc.", "Inc.", "Ltd.", "Corp.", "Dept.", "Univ.",
    "Ave.", "Blvd.", "e.g.", "i.e.", "a.m.", "p.m.",
];

impl SentenceDetector {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    /// Process new text, returning (complete_sentences, remaining_fragment).
    pub fn process(&mut self, text: &str) -> (Vec<String>, String) {
        self.buffer.push_str(text);

        let mut sentences = Vec::new();
        let mut start = 0;
        let chars: Vec<char> = self.buffer.chars().collect();
        let len = chars.len();

        let mut i = 0;
        while i < len {
            if is_terminal(chars[i]) {
                // Check for ellipsis
                if chars[i] == '.' && i + 2 < len && chars[i + 1] == '.' && chars[i + 2] == '.' {
                    i += 3;
                    continue;
                }

                // Check for abbreviation
                let preceding = &self.buffer[start..self.buffer.char_indices()
                    .nth(i + 1)
                    .map(|(idx, _)| idx)
                    .unwrap_or(self.buffer.len())];

                if chars[i] == '.' && is_abbreviation(preceding) {
                    i += 1;
                    continue;
                }

                // Found a sentence boundary
                let end_byte = self.buffer.char_indices()
                    .nth(i + 1)
                    .map(|(idx, _)| idx)
                    .unwrap_or(self.buffer.len());

                let sentence = self.buffer[start..end_byte].trim().to_string();
                if !sentence.is_empty() {
                    sentences.push(sentence);
                }

                start = end_byte;
            }
            i += 1;
        }

        // Keep the remaining fragment
        let remaining = self.buffer[start..].trim().to_string();
        self.buffer = remaining.clone();

        (sentences, remaining)
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
    }

    /// Flush any remaining buffer as a sentence.
    pub fn flush(&mut self) -> Option<String> {
        let text = self.buffer.trim().to_string();
        self.buffer.clear();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }
}

fn is_terminal(c: char) -> bool {
    matches!(c, '.' | '?' | '!')
}

fn is_abbreviation(text: &str) -> bool {
    let trimmed = text.trim();
    ABBREVIATIONS.iter().any(|abbr| trimmed.ends_with(abbr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_sentence() {
        let mut sd = SentenceDetector::new();
        let (sentences, remaining) = sd.process("Hello world. How are you?");
        assert_eq!(sentences, vec!["Hello world.", "How are you?"]);
        assert_eq!(remaining, "");
    }

    #[test]
    fn test_abbreviation() {
        let mut sd = SentenceDetector::new();
        let (sentences, remaining) = sd.process("Talk to Dr. Smith about it.");
        assert_eq!(sentences, vec!["Talk to Dr. Smith about it."]);
        assert_eq!(remaining, "");
    }

    #[test]
    fn test_ellipsis() {
        let mut sd = SentenceDetector::new();
        let (sentences, remaining) = sd.process("Well... I think so.");
        assert_eq!(sentences, vec!["Well... I think so."]);
        assert_eq!(remaining, "");
    }

    #[test]
    fn test_fragment() {
        let mut sd = SentenceDetector::new();
        let (sentences, remaining) = sd.process("Hello world");
        assert!(sentences.is_empty());
        assert_eq!(remaining, "Hello world");
    }
}
