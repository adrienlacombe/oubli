use crate::error::BackupError;

/// How many words per display group.
const GROUP_SIZE: usize = 4;
/// How many random words to verify.
const VERIFY_COUNT: usize = 3;

/// A group of words to show the user during seed backup.
#[derive(Debug, Clone)]
pub struct WordGroup {
    /// 0-based index of the first word in this group.
    pub start_index: usize,
    /// The words in this group.
    pub words: Vec<String>,
}

/// A prompt asking the user to confirm a specific word.
#[derive(Debug, Clone)]
pub struct VerificationPrompt {
    /// 1-based word position shown to the user ("Word #5").
    pub word_number: usize,
    /// The expected answer.
    expected: String,
}

impl VerificationPrompt {
    /// Check the user's answer (case-insensitive, trimmed).
    pub fn check(&self, answer: &str) -> bool {
        self.expected.eq_ignore_ascii_case(answer.trim())
    }

    /// The word position (1-based) the user must recall.
    pub fn word_number(&self) -> usize {
        self.word_number
    }
}

/// Drives the seed phrase backup display and verification flow.
pub struct SeedDisplayFlow {
    words: Vec<String>,
}

impl SeedDisplayFlow {
    /// Create a new flow from a mnemonic phrase (space-separated words).
    pub fn new(mnemonic: &str) -> Result<Self, BackupError> {
        let words: Vec<String> = mnemonic.split_whitespace().map(String::from).collect();
        if words.is_empty() {
            return Err(BackupError::SeedDisplay("empty mnemonic".into()));
        }
        Ok(Self { words })
    }

    /// Split the mnemonic into groups of 4 for display.
    pub fn word_groups(&self) -> Vec<WordGroup> {
        self.words
            .chunks(GROUP_SIZE)
            .enumerate()
            .map(|(i, chunk)| WordGroup {
                start_index: i * GROUP_SIZE,
                words: chunk.to_vec(),
            })
            .collect()
    }

    /// Generate verification prompts — pick `VERIFY_COUNT` positions deterministically
    /// spread across the word list.
    pub fn verification_prompts(&self) -> Vec<VerificationPrompt> {
        let len = self.words.len();
        if len == 0 {
            return vec![];
        }
        let step = len / (VERIFY_COUNT + 1);
        (1..=VERIFY_COUNT)
            .map(|i| {
                let idx = (step * i).min(len - 1);
                VerificationPrompt {
                    word_number: idx + 1,
                    expected: self.words[idx].clone(),
                }
            })
            .collect()
    }

    /// Verify all prompts against user answers. Returns first failure if any.
    pub fn verify_all(
        &self,
        prompts: &[VerificationPrompt],
        answers: &[&str],
    ) -> Result<(), BackupError> {
        for (prompt, answer) in prompts.iter().zip(answers.iter()) {
            if !prompt.check(answer) {
                return Err(BackupError::VerificationFailed {
                    position: prompt.word_number,
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_MNEMONIC: &str =
        "abandon ability able about above absent absorb abstract absurd abuse access accident";

    #[test]
    fn word_groups_of_four() {
        let flow = SeedDisplayFlow::new(TEST_MNEMONIC).unwrap();
        let groups = flow.word_groups();
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].words.len(), 4);
        assert_eq!(groups[0].start_index, 0);
        assert_eq!(groups[1].start_index, 4);
        assert_eq!(groups[2].start_index, 8);
        assert_eq!(groups[0].words[0], "abandon");
        assert_eq!(groups[2].words[3], "accident");
    }

    #[test]
    fn verification_prompts_spread() {
        let flow = SeedDisplayFlow::new(TEST_MNEMONIC).unwrap();
        let prompts = flow.verification_prompts();
        assert_eq!(prompts.len(), 3);
        // All positions should be different
        let positions: Vec<usize> = prompts.iter().map(|p| p.word_number()).collect();
        assert!(positions[0] < positions[1]);
        assert!(positions[1] < positions[2]);
    }

    #[test]
    fn verification_correct() {
        let flow = SeedDisplayFlow::new(TEST_MNEMONIC).unwrap();
        let prompts = flow.verification_prompts();
        let words: Vec<String> = TEST_MNEMONIC.split_whitespace().map(String::from).collect();
        let answers: Vec<&str> = prompts
            .iter()
            .map(|p| words[p.word_number() - 1].as_str())
            .collect();
        assert!(flow.verify_all(&prompts, &answers).is_ok());
    }

    #[test]
    fn verification_wrong_answer() {
        let flow = SeedDisplayFlow::new(TEST_MNEMONIC).unwrap();
        let prompts = flow.verification_prompts();
        let answers = vec!["wrong", "wrong", "wrong"];
        assert!(flow.verify_all(&prompts, &answers).is_err());
    }

    #[test]
    fn verification_case_insensitive() {
        let prompt = VerificationPrompt {
            word_number: 1,
            expected: "abandon".to_string(),
        };
        assert!(prompt.check("ABANDON"));
        assert!(prompt.check("  Abandon  "));
    }
}
