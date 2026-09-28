use std::collections::HashMap;

/// The result of tokenizing a passage
/// - term_counts = distinct terms mapped to their local doc freq
/// - doc_len = total number of VALID indexed tokens in the passage
pub struct TokenizedDoc {
    pub term_counts: HashMap<String, u32>,
    pub doc_len: u32,
}

///Helper: returns true if the character is  not alphanumeric
fn is_delimiter(c: char) -> bool {
    return !c.is_alphanumeric();
}

///Helper: counts how many digits are in a word
fn count_digits(word: &str) -> usize {
    let mut digits = 0;
    for c in word.chars() {
        if c.is_ascii_digit() {
            digits += 1
        }
    }
    return digits;
}

/// Cleans and tokenizes the passage based on requirements:
/// - lowercase
/// - split on non-alphanueric characters
/// - length between 2 and 30
/// - at most 5 digits
pub fn tokenize(text: &str) -> TokenizedDoc {
    let mut term_counts: HashMap<String, u32> = HashMap::new();
    let mut doc_len: u32 = 0;

    //split on any character that is not alphanumeric
    for raw_token in text.split(is_delimiter) {
        if raw_token.is_empty() {
            continue;
        }
        let char_count = raw_token.chars().count();
        if (char_count < 2 || char_count > 30) || (count_digits(raw_token) > 5) {
            continue;
        }
        let term = raw_token.to_lowercase();
        let count = term_counts.entry(term).or_insert(0);
        *count += 1;
        doc_len += 1;
    }
    return TokenizedDoc {
        term_counts,
        doc_len,
    };
}
