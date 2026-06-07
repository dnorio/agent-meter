/// Estimate input tokens from request bytes if explicit count not provided.
/// Uses ~3.5 chars/token ratio (average across GPT/Claude tokenizers).
pub fn estimate_input_tokens(request_bytes: Option<i32>, explicit: Option<i32>) -> Option<i32> {
    explicit.or_else(|| request_bytes.map(|b| (b as f64 / 3.5) as i32))
}

/// Estimate output tokens from response bytes if explicit count not provided.
pub fn estimate_output_tokens(response_bytes: Option<i32>, explicit: Option<i32>) -> Option<i32> {
    explicit.or_else(|| response_bytes.map(|b| (b as f64 / 3.5) as i32))
}

pub fn estimate_total(input: Option<i32>, output: Option<i32>) -> Option<i32> {
    match (input, output) {
        (Some(i), Some(o)) => Some(i + o),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_input_tokens_prefers_explicit() {
        assert_eq!(estimate_input_tokens(Some(100), Some(50)), Some(50));
    }

    #[test]
    fn test_estimate_input_tokens_from_bytes() {
        // 100 bytes / 3.5 = 28 tokens
        assert_eq!(estimate_input_tokens(Some(100), None), Some(28));
    }

    #[test]
    fn test_estimate_input_tokens_none() {
        assert_eq!(estimate_input_tokens(None, None), None);
    }

    #[test]
    fn test_estimate_output_tokens_prefers_explicit() {
        assert_eq!(estimate_output_tokens(Some(100), Some(50)), Some(50));
    }

    #[test]
    fn test_estimate_output_tokens_from_bytes() {
        // 200 bytes / 3.5 = 57 tokens
        assert_eq!(estimate_output_tokens(Some(200), None), Some(57));
    }

    #[test]
    fn test_estimate_total() {
        assert_eq!(estimate_total(Some(25), Some(50)), Some(75));
    }

    #[test]
    fn test_estimate_total_partial() {
        assert_eq!(estimate_total(Some(25), None), None);
    }
}
