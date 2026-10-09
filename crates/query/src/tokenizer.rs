//! Query tokenizer
//! Here is where I will clean and tokenize the query into distinct and valid search terms
//! This will apply the same rules as the parser tokenizer to ensure the terms match with terms stored in the lexicon
//!

fn is_delimeter(c: char) -> bool {
    return !c.is_alphanumeric();
}

/// Counts how many ascii digits appear in a token
fn count_digits(word: &str) -> usize {
    let mut digits = 0;
    for c in word.chars() {
        if c.is_ascii_digit() {
            digits += 1;
        }
    }
    return digits;
}

/// Cleans and extracts unique valid query terms form a raw user input
///
/// Arguments:
///     raw_query: The raw string
///
/// Returns
///     Vec<String> containing distinct lowercased valid terms
pub fn tokenize_query(raw_query: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    //split on any chracter that is not alphanumeric
    for raw_token in raw_query.split(is_delimeter) {
        if raw_token.is_empty() {
            continue;
        }
        let char_count = raw_token.chars().count();
        //discard tokens that do not match the index criteria
        if char_count < 2 || char_count > 30 || count_digits(raw_token) > 5 {
            continue;
        }
        let clean_term = raw_token.to_lowercase();
        //changed it ot check directly I mean rather than allocating a hashset for a small query is unnecessary overhead compared to a checking the array directly
        if !terms.contains(&clean_term) {
            terms.push(clean_term);
        }
    }
    return terms;
}
