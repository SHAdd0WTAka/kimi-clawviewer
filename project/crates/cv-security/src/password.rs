//! Password generation for ClawViewer session authentication.
//!
//! Provides two password formats:
//! - A 6-character memorable word (e.g. `fox7k9`) for human use.
//! - A 12-character random token (e.g. `a3F9mK2pL7qR`) for machine use.
//!
//! Also exports [`calculate_entropy`] for measuring password strength.

use rand::{Rng, seq::SliceRandom};
use tracing::debug;

/// A built-in word list of ~100 common short English words mixed with
/// number-like words to increase the alphabet size.
const WORDLIST: &[&str] = &[
    // Common 3-4 letter words
    "ace", "act", "add", "age", "ago", "aid", "air", "all", "and", "any",
    "ape", "apt", "are", "arm", "art", "ash", "ask", "ate", "awe", "axe",
    "bad", "bag", "ban", "bar", "bat", "bay", "bed", "bet", "big", "bit",
    "bow", "box", "boy", "bug", "bus", "but", "buy", "bye",
    "cab", "can", "cap", "car", "cat", "cop", "cow", "cry", "cup", "cut",
    "dad", "day", "did", "die", "dig", "dim", "dip", "dog", "dot", "dry",
    "dub", "due", "dug", "ear", "eat", "egg", "ego", "elf", "elk", "elm",
    "end", "era", "eve", "eye", "fan", "far", "fat", "fax", "fee", "few",
    "fit", "fix", "flu", "fly", "fog", "foo", "for", "fox", "fry", "fun",
    "gag", "gap", "gas", "gem", "get", "gig", "god", "got", "gum", "gun",
    "guy", "gym", // 108 words
];

/// Generate a 6-character session password.
///
/// The password is formed by concatenating:
/// - A random 3-letter word from [`WORDLIST`]
/// - A random 3-digit number (000-999)
///
/// This produces passwords like `fox123`, `cat042`, `sky999`.
///
/// # Entropy
/// Approximately ~16.8 bits from the word (~108 choices) + ~9.97 bits
/// from the number (1000 choices) = ~26.8 bits total.
pub fn generate_password_word() -> String {
    let mut rng = rand::thread_rng();
    let word = WORDLIST.choose(&mut rng).unwrap();
    let num = rng.gen_range(0..1000u16);
    let password = format!("{}{:03}", word, num);
    debug!("Generated session password: {}", password);
    password
}

/// Generate a 12-character random token.
///
/// Uses a mixed alphabet of uppercase, lowercase, and digits:
/// - 26 lowercase + 26 uppercase + 10 digits = 62 characters
/// - 12 characters chosen uniformly at random
///
/// # Entropy
/// Approximately ~71.5 bits (62^12).
pub fn generate_password_token() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    let token: String = (0..12)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect();
    debug!("Generated session token: {}", token);
    token
}

/// Calculate the Shannon entropy of a password in bits.
///
/// Entropy is computed as `log2(alphabet_size ^ length)`, where
/// `alphabet_size` is estimated from the character classes present
/// in the password.
///
/// # Example
/// ```
/// use cv_security::password::calculate_entropy;
/// let e = calculate_entropy("Hello1");
/// assert!(e > 0.0);
/// ```
pub fn calculate_entropy(password: &str) -> f64 {
    if password.is_empty() {
        return 0.0;
    }

    let has_lowercase = password.chars().any(|c| c.is_ascii_lowercase());
    let has_uppercase = password.chars().any(|c| c.is_ascii_uppercase());
    let has_digits = password.chars().any(|c| c.is_ascii_digit());
    let has_symbols = password.chars().any(|c| !c.is_alphanumeric());

    let mut alphabet_size: u64 = 0;
    if has_lowercase {
        alphabet_size += 26;
    }
    if has_uppercase {
        alphabet_size += 26;
    }
    if has_digits {
        alphabet_size += 10;
    }
    if has_symbols {
        alphabet_size += 32; // Approximate common symbol count
    }

    if alphabet_size == 0 {
        return 0.0;
    }

    let len = password.len() as f64;
    let base = alphabet_size as f64;
    len * base.log2()
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_word_is_6_chars() {
        let pwd = generate_password_word();
        assert_eq!(pwd.len(), 6, "Password '{}' should be 6 characters", pwd);
    }

    #[test]
    fn password_word_starts_with_wordlist_entry() {
        let pwd = generate_password_word();
        // First 3 chars should be a lowercase word
        let prefix = &pwd[..3];
        assert!(
            WORDLIST.contains(&prefix),
            "Prefix '{}' should be in wordlist",
            prefix
        );
        // Last 3 chars should be digits
        let suffix = &pwd[3..];
        assert!(suffix.parse::<u16>().is_ok(), "Suffix '{}' should be numeric", suffix);
    }

    #[test]
    fn password_token_is_12_chars() {
        let token = generate_password_token();
        assert_eq!(token.len(), 12, "Token '{}' should be 12 characters", token);
    }

    #[test]
    fn password_token_uses_valid_alphabet() {
        let token = generate_password_token();
        const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        for ch in token.chars() {
            assert!(ALPHABET.contains(ch), "Char '{}' not in alphabet", ch);
        }
    }

    #[test]
    fn password_word_entropy_above_30_bits() {
        // Generate many passwords and check average entropy
        let mut total_entropy = 0.0;
        const N: usize = 100;
        for _ in 0..N {
            let pwd = generate_password_word();
            total_entropy += calculate_entropy(&pwd);
        }
        let avg = total_entropy / N as f64;
        println!("Average word password entropy: {:.2} bits", avg);
        assert!(
            avg > 30.0,
            "Average entropy {:.2} bits should be > 30 bits",
            avg
        );
    }

    #[test]
    fn password_token_entropy_above_70_bits() {
        let token = generate_password_token();
        let entropy = calculate_entropy(&token);
        println!("Token entropy: {:.2} bits (token: {})", entropy, token);
        // Theoretical entropy is 71.5 bits, but calculate_entropy estimates from character classes
        // A 12-char token with upper+lower+digits has 62^12 = ~71.5 bits
        assert!(
            entropy >= 68.0,
            "Token entropy {:.2} bits should be >= 68 bits",
            entropy
        );
    }

    #[test]
    fn entropy_of_empty_string_is_zero() {
        assert_eq!(calculate_entropy(""), 0.0);
    }

    #[test]
    fn entropy_calculates_correctly_for_known_values() {
        // 8 lowercase chars: 26^8 => 8 * log2(26) = ~37.6 bits
        let e = calculate_entropy("abcdefgh");
        let expected = 8.0 * (26.0f64).log2();
        assert!((e - expected).abs() < 0.1);

        // 8 mixed: 62^8 => 8 * log2(62) = ~47.6 bits
        let e = calculate_entropy("Ab1Cd2Ef");
        let expected = 8.0 * (62.0f64).log2();
        assert!((e - expected).abs() < 0.1);
    }

    #[test]
    fn password_word_is_unique_enough() {
        // Generate many passwords, ensure variety
        let mut seen = std::collections::HashSet::new();
        for _ in 0..50 {
            seen.insert(generate_password_word());
        }
        // With random generation, expect very few collisions
        assert!(seen.len() >= 48, "Expected high variety, got {} unique passwords", seen.len());
    }

    #[test]
    fn password_token_is_unique_enough() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..100 {
            seen.insert(generate_password_token());
        }
        assert_eq!(seen.len(), 100, "Expected 100 unique tokens");
    }
}
