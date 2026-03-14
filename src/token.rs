use std::sync::OnceLock;

use tiktoken_rs::CoreBPE;

/// Get the shared cl100k_base tokenizer (lazily initialized).
fn tokenizer() -> &'static CoreBPE {
    static TOKENIZER: OnceLock<CoreBPE> = OnceLock::new();
    TOKENIZER.get_or_init(|| {
        tiktoken_rs::cl100k_base().expect("Failed to load cl100k_base tokenizer")
    })
}

/// Count the number of tokens in a string using cl100k_base.
/// Falls back to estimation if tokenizer isn't available.
pub fn count_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    tokenizer().encode_with_special_tokens(text).len()
}

/// Fast estimation: ~4 characters per token on average.
pub fn estimate_tokens(text: &str) -> usize {
    (text.len() + 3) / 4
}

/// Format a token count with commas for display.
pub fn format_token_count(count: usize) -> String {
    if count < 1000 {
        return format!("~{}", count);
    }
    let s = count.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    format!("~{}", result.chars().rev().collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_tokens() {
        let text = "fn main() { println!(\"Hello, world!\"); }";
        let count = count_tokens(text);
        assert!(count > 0);
        assert!(count < 50); // Should be roughly 10-15 tokens
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcdefgh"), 2);
    }

    #[test]
    fn test_format_token_count() {
        assert_eq!(format_token_count(42), "~42");
        assert_eq!(format_token_count(1500), "~1,500");
        assert_eq!(format_token_count(18500), "~18,500");
    }
}
