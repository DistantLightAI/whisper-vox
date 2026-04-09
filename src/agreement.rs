use std::collections::VecDeque;

/// LocalAgreement-n policy: text emitted only when N consecutive
/// transcription passes agree on a word-level prefix.
pub struct LocalAgreement {
    n: usize,
    history: VecDeque<Vec<String>>,
    confirmed_len: usize,
}

impl LocalAgreement {
    pub fn new(n: usize) -> Self {
        Self {
            n: n.max(1),
            history: VecDeque::new(),
            confirmed_len: 0,
        }
    }

    /// Feed a new transcription result. Returns confirmed NEW words (delta) or None.
    pub fn process(&mut self, text: &str) -> Option<String> {
        // Lowercase at insertion to avoid repeated to_lowercase() in comparisons
        let words: Vec<String> = text.split_whitespace().map(|w| w.to_lowercase()).collect();
        self.history.push_back(words);

        // Keep only the last N entries
        while self.history.len() > self.n {
            self.history.pop_front();
        }

        if self.history.len() < self.n {
            return None;
        }

        // Find longest common word-level prefix among last N entries
        let common_len = self.common_prefix_len();

        if common_len > self.confirmed_len {
            // Emit the delta (new confirmed words)
            let latest = &self.history[self.history.len() - 1];
            let delta: Vec<&str> = latest[self.confirmed_len..common_len]
                .iter()
                .map(|s| s.as_str())
                .collect();
            self.confirmed_len = common_len;
            Some(delta.join(" "))
        } else {
            None
        }
    }

    /// Reset state between speech segments.
    pub fn reset(&mut self) {
        self.history.clear();
        self.confirmed_len = 0;
    }

    fn common_prefix_len(&self) -> usize {
        if self.history.is_empty() {
            return 0;
        }

        let min_len = self.history.iter().map(|w| w.len()).min().unwrap_or(0);
        let mut common = 0;

        for i in 0..min_len {
            let word = &self.history[0][i];
            if self.history.iter().all(|entry| {
                i < entry.len() && entry[i] == *word
            }) {
                common = i + 1;
            } else {
                break;
            }
        }

        common
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agreement_basic() {
        let mut ag = LocalAgreement::new(2);
        assert_eq!(ag.process("hello world"), None);
        assert_eq!(ag.process("hello world today"), Some("hello world".into()));
    }

    #[test]
    fn test_agreement_no_match() {
        let mut ag = LocalAgreement::new(2);
        assert_eq!(ag.process("hello"), None);
        assert_eq!(ag.process("goodbye"), None);
    }

    #[test]
    fn test_agreement_incremental() {
        let mut ag = LocalAgreement::new(2);
        ag.process("the quick");
        ag.process("the quick brown");
        // "the quick" confirmed
        assert_eq!(ag.process("the quick brown fox"), Some("brown".into()));
    }

    #[test]
    fn test_reset() {
        let mut ag = LocalAgreement::new(2);
        ag.process("hello world");
        ag.process("hello world");
        ag.reset();
        assert_eq!(ag.process("new text"), None);
    }
}
