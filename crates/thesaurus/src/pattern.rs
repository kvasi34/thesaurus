const MAX_DEPTH: u16 = 1000;

/// Glob-style matcher.
///
/// Supported glob-style patterns:
///
/// h?llo matches hello, hallo and hxllo
/// h*llo matches hllo and heeeello
/// h[ae]llo matches hello and hallo, but not hillo
/// h[^e]llo matches hallo, hbllo, ... but not hello
/// h[a-b]llo matches hallo and hbllo
///
/// The `\` symbol is used to escape special characters and match the verbatim.
///
/// Matching is byte-oriented and case-sensitive, exactly as Redis does it: keys are byte
/// strings with no encoding awareness, so `?` matches a single *byte* and a multi-byte
/// UTF-8 character takes as many `?` as it has bytes.
pub fn string_match(string: &str, pattern: &[u8]) -> bool {
    let mut skip_longer_matches = false;
    string_match_impl(string.as_bytes(), pattern, &mut skip_longer_matches, 0)
}

fn string_match_impl(
    mut string: &[u8],
    mut pattern: &[u8],
    skip_longer_matches: &mut bool,
    depth: u16,
) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }

    // Iterate over the pattern and string sequences
    while !pattern.is_empty() && !string.is_empty() {
        // Consume the pattern character
        match pattern[0] {
            b'*' => {
                // A run of stars is equal to a single one
                while pattern.len() > 1 && pattern[1] == b'*' {
                    pattern = &pattern[1..];
                }

                // A trailing star matches everything
                if pattern.len() == 1 {
                    return true;
                }

                // Try the rest of the pattern at every remaining offset of the string
                while !string.is_empty() {
                    if string_match_impl(string, &pattern[1..], skip_longer_matches, depth + 1) {
                        return true;
                    }

                    if *skip_longer_matches {
                        return false;
                    }

                    string = &string[1..];
                }

                // The rest of the pattern matches nowhere in the rest of the string. An earlier
                // star could only push the rest of the pattern further right, so letting it try
                // longer matches is pointless: unwind the whole search.
                *skip_longer_matches = true;
                return false;
            }
            b'?' => {}
            b'[' => {
                pattern = &pattern[1..];
                let negated = pattern.first() == Some(&b'^');
                if negated {
                    pattern = &pattern[1..];
                }

                // Walk the class to its closing bracket, recording whether any of its members
                // matched the single string character the class consumes
                let mut matched = false;
                loop {
                    if pattern.len() > 1 && pattern[0] == b'\\' {
                        // The escape applies inside a class too
                        pattern = &pattern[1..];
                        matched |= pattern[0] == string[0];
                    } else if pattern.first() == Some(&b']') {
                        break;
                    } else if pattern.is_empty() {
                        // An unterminated class ends where the pattern does. There is no closing
                        // bracket to step over below, so leave the pattern empty.
                        break;
                    } else if pattern.len() > 2 && pattern[1] == b'-' {
                        // A reversed range is swapped rather than rejected
                        let (mut start, mut end) = (pattern[0], pattern[2]);
                        if start > end {
                            std::mem::swap(&mut start, &mut end);
                        }

                        matched |= (start..=end).contains(&string[0]);
                        pattern = &pattern[2..];
                    } else {
                        matched |= pattern[0] == string[0];
                    }

                    pattern = &pattern[1..];
                }

                if matched == negated {
                    return false;
                }
            }
            // A trailing backslash has nothing to escape, so it falls through to the arm below
            // and matches a verbatim backslash
            b'\\' if pattern.len() > 1 => {
                pattern = &pattern[1..];
                if pattern[0] != string[0] {
                    return false;
                }
            }
            _ => {
                if pattern[0] != string[0] {
                    return false;
                }
            }
        }

        // Move to the next pattern character
        if !pattern.is_empty() {
            pattern = &pattern[1..];
        }

        // Move to the next string character
        if !string.is_empty() {
            string = &string[1..];
        }

        if string.is_empty() {
            // Trailing stars still match the now-empty remainder of the string.
            while pattern.first() == Some(&b'*') {
                pattern = &pattern[1..];
            }

            break;
        }
    }

    pattern.is_empty() && string.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_question_mark_matches_single_character() {
        assert!(string_match("hello", b"h?llo"));
        assert!(string_match("hallo", b"h?llo"));
        assert!(string_match("hxllo", b"h?llo"));
    }

    #[test]
    fn test_star_matches_any_sequence() {
        assert!(string_match("hllo", b"h*llo"));
        assert!(string_match("heeeello", b"h*llo"));
    }

    #[test]
    fn test_character_class_matches_one_of_the_set() {
        assert!(string_match("hello", b"h[ae]llo"));
        assert!(string_match("hallo", b"h[ae]llo"));
        assert!(!string_match("hillo", b"h[ae]llo"));
    }

    #[test]
    fn test_negated_character_class_matches_anything_but_the_set() {
        assert!(string_match("hallo", b"h[^e]llo"));
        assert!(string_match("hbllo", b"h[^e]llo"));
        assert!(!string_match("hello", b"h[^e]llo"));
    }

    #[test]
    fn test_character_range_matches_one_of_the_range() {
        assert!(string_match("hallo", b"h[a-b]llo"));
        assert!(string_match("hbllo", b"h[a-b]llo"));
        assert!(!string_match("hcllo", b"h[a-b]llo"));
    }

    #[test]
    fn test_literal_pattern() {
        assert!(string_match("foo", b"foo"));
        assert!(!string_match("bar", b"foo"));
    }

    #[test]
    fn test_pattern_must_consume_the_whole_string() {
        // The match fails unless both sides run out together, so a pattern that describes only
        // a prefix of the key is not a match.
        assert!(!string_match("foobar", b"foo"));
        assert!(!string_match("hello world", b"hello"));
        assert!(!string_match("a", b""));
        assert!(string_match("", b""));
    }

    #[test]
    fn test_star_matches_an_empty_sequence() {
        assert!(string_match("foo", b"foo*"));
        assert!(string_match("foo", b"*foo"));
        assert!(string_match("aaa", b"*a"));
        assert!(string_match("user:1000:name", b"user:*:name"));
        assert!(!string_match("user:1000:mail", b"user:*:name"));
    }

    #[test]
    fn test_star_does_not_match_the_empty_string() {
        // Redis' matcher only loops while both sides have input left, so an empty key never
        // reaches the star. `KEYS *` still reports empty keys, but through the caller's
        // all-keys shortcut rather than through here — see `Store::keys`.
        assert!(!string_match("", b"*"));
        assert!(!string_match("", b"**"));
        assert!(!string_match("", b"?"));
    }

    #[test]
    fn test_consecutive_stars_are_collapsed() {
        // Redis has no path components, so `**` carries no meaning beyond `*`. POSIX glob
        // implementations reject `a**b` as a misplaced recursive wildcard instead.
        assert!(string_match("ab", b"a**b"));
        assert!(string_match("axyzb", b"a**b"));
        assert!(!string_match("axyzc", b"a**b"));
    }

    #[test]
    fn test_backtracking_across_several_stars() {
        assert!(string_match("hello world", b"h*o*d"));
        assert!(!string_match("hello world", b"h*o*z"));
        assert!(string_match("aaaaaaaab", b"a*a*a*b"));
        assert!(!string_match("aaaaaaaac", b"a*a*a*b"));
    }

    #[test]
    fn test_exclamation_mark_is_not_negation() {
        // POSIX glob negates a class with `[!abc]`; for Redis `!` is an ordinary member of it.
        assert!(string_match("!", b"[!a]"));
        assert!(string_match("a", b"[!a]"));
        assert!(!string_match("b", b"[!a]"));
    }

    #[test]
    fn test_reversed_range_is_swapped() {
        assert!(string_match("b", b"[c-a]"));
        assert!(!string_match("d", b"[c-a]"));
    }

    #[test]
    fn test_empty_class_matches_nothing_but_consumes_a_character() {
        assert!(!string_match("a", b"[]"));
        assert!(!string_match("", b"[]"));
    }

    #[test]
    fn test_unterminated_class_ends_at_the_end_of_the_pattern() {
        // `[foo` is not an error: it is the class {f, o} applied to a single character.
        assert!(string_match("f", b"[foo"));
        assert!(string_match("o", b"[foo"));
        assert!(!string_match("x", b"[foo"));
        assert!(!string_match("foo", b"[foo"));
    }

    #[test]
    fn test_backslash_escapes_special_characters() {
        assert!(string_match("*", b"\\*"));
        assert!(!string_match("abc", b"\\*"));
        assert!(string_match("?", b"\\?"));
        assert!(string_match("[", b"\\["));
        assert!(string_match("foo*bar", b"foo\\*bar"));
        assert!(!string_match("fooXbar", b"foo\\*bar"));
    }

    #[test]
    fn test_backslash_escapes_inside_a_class() {
        assert!(string_match("]", b"[\\]]"));
        assert!(string_match("-", b"[a\\-c]"));
        assert!(!string_match("b", b"[a\\-c]"));
    }

    #[test]
    fn test_trailing_backslash_matches_itself() {
        // There is nothing left to escape, so the backslash is matched verbatim.
        assert!(string_match("\\", b"\\"));
        assert!(!string_match("a", b"\\"));
    }

    #[test]
    fn test_matching_is_byte_oriented() {
        // "é" is two bytes in UTF-8, so it takes two `?` to match — as in Redis, whose keys are
        // byte strings with no character-encoding awareness.
        assert!(!string_match("é", b"?"));
        assert!(string_match("é", b"??"));
        assert!(string_match("é", b"*"));
        assert!(!string_match("é", b"[a-z]"));
    }

    #[test]
    fn test_matching_is_case_sensitive() {
        assert!(!string_match("FOO", b"foo"));
        assert!(!string_match("A", b"[a-z]"));
    }

    #[test]
    fn test_recursion_depth_is_capped() {
        // Past the limit the match is reported as a non-match. The point of the test is that a
        // pattern taken from an untrusted client terminates instead of overflowing the stack.
        let pattern = "*a".repeat(MAX_DEPTH as usize + 100).into_bytes();
        let string = "a".repeat(MAX_DEPTH as usize + 100);

        assert!(!string_match(&string, &pattern));
    }
}
