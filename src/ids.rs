/// Shared identifier rendering helpers (AD-009, docs/architecture/identifiers.md).
///
/// Contract-issued counters are typed `u64` on-chain and rendered as a
/// zero-padded 12-digit decimal string at the API boundary:
/// `va:verification:<12-digit>`, `va:escrow:<12-digit>`, `va:financing:<12-digit>`.
/// Renders a contract-issued `u64` counter as the presentation form
/// `va:{prefix}:{counter:012}` (e.g. `va:verification:000000000042`).
pub fn counter_id(prefix: &str, counter: u64) -> String {
    format!("{prefix}:{counter:012}")
}

/// Parses a presentation-form counter ID back to its `u64` counter.
/// Accepts `va:verification:000000000042` or a bare `000000000042`.
pub fn parse_counter_id(input: &str, prefix: &str) -> Option<u64> {
    let bare = input.strip_prefix(&format!("{prefix}:"))?;
    bare.parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use crate::ids::{counter_id, parse_counter_id};

    #[test]
    fn renders_zero_padded_counter() {
        assert_eq!(
            counter_id("va:verification", 42),
            "va:verification:000000000042"
        );
        assert_eq!(counter_id("va:escrow", 1), "va:escrow:000000000001");
    }

    #[test]
    fn parses_presentation_form() {
        assert_eq!(
            parse_counter_id("va:verification:000000000042", "va:verification"),
            Some(42)
        );
        assert_eq!(
            parse_counter_id("va:escrow:000000000011", "va:escrow"),
            Some(11)
        );
    }

    #[test]
    fn rejects_wrong_prefix() {
        assert_eq!(
            parse_counter_id("va:escrow:000000000042", "va:verification"),
            None
        );
    }
}
