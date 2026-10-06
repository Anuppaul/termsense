#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuoteStyle {
    None,
    Single,
    Double,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) text: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) quote: QuoteStyle,
}

pub(crate) fn active_segment_tokens(buffer: &str, cursor: usize) -> Vec<Token> {
    let start = active_segment_start(buffer, cursor);
    tokens_in_range(buffer, start, cursor)
}

pub(crate) fn tokens_before_cursor(buffer: &str, cursor: usize) -> Vec<Token> {
    tokens_in_range(buffer, 0, cursor)
}

pub(crate) fn active_segment_start(buffer: &str, cursor: usize) -> usize {
    let before = &buffer[..cursor];
    let mut quote = QuoteStyle::None;
    let mut escaped = false;
    let mut last_boundary = 0;

    let mut iter = before.char_indices().peekable();
    while let Some((offset, ch)) = iter.next() {
        if escaped {
            escaped = false;
            continue;
        }

        match quote {
            QuoteStyle::Single => {
                if ch == '\'' {
                    quote = QuoteStyle::None;
                }
                continue;
            }
            QuoteStyle::Double => {
                if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    quote = QuoteStyle::None;
                }
                continue;
            }
            QuoteStyle::None => {}
        }

        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '\'' {
            quote = QuoteStyle::Single;
            continue;
        }
        if ch == '"' {
            quote = QuoteStyle::Double;
            continue;
        }

        if matches!(ch, ';' | '|' | '&') {
            let mut boundary = offset + ch.len_utf8();

            if matches!(ch, '|' | '&') {
                if let Some((next_offset, next)) = iter.peek().copied() {
                    if next == ch {
                        iter.next();
                        boundary = next_offset + next.len_utf8();
                    }
                }
            }

            last_boundary = boundary;
        }
    }

    let mut start = last_boundary;
    while start < cursor {
        let Some(ch) = buffer[start..cursor].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        start += ch.len_utf8();
    }
    start
}

fn tokens_in_range(buffer: &str, start: usize, cursor: usize) -> Vec<Token> {
    let before = &buffer[start..cursor];
    let mut tokens = Vec::new();
    let mut iter = before.char_indices().peekable();

    while let Some((relative_index, ch)) = iter.peek().copied() {
        if ch.is_whitespace() {
            iter.next();
            continue;
        }

        let raw_start = start + relative_index;
        let mut replacement_start = raw_start;
        let mut end = raw_start;
        let mut text = String::new();
        let mut quote = QuoteStyle::None;
        let mut active_quote = QuoteStyle::None;

        if ch == '\'' || ch == '"' {
            iter.next();
            replacement_start = raw_start + ch.len_utf8();
            end = replacement_start;
            quote = if ch == '\'' {
                QuoteStyle::Single
            } else {
                QuoteStyle::Double
            };
            active_quote = quote;
        }

        while let Some((relative_offset, current)) = iter.peek().copied() {
            if active_quote == QuoteStyle::None && current.is_whitespace() {
                break;
            }

            if active_quote == QuoteStyle::None && matches!(current, ';' | '|' | '&') {
                break;
            }

            iter.next();
            let absolute_offset = start + relative_offset;
            end = absolute_offset + current.len_utf8();

            match active_quote {
                QuoteStyle::Single => {
                    if current == '\'' {
                        active_quote = QuoteStyle::None;
                        end = absolute_offset;
                        break;
                    }
                    text.push(current);
                }
                QuoteStyle::Double => {
                    if current == '"' {
                        active_quote = QuoteStyle::None;
                        end = absolute_offset;
                        break;
                    }
                    if current == '\\' {
                        if let Some((next_relative, next)) = iter.peek().copied() {
                            iter.next();
                            end = start + next_relative + next.len_utf8();
                            text.push(next);
                        } else {
                            text.push(current);
                        }
                    } else {
                        text.push(current);
                    }
                }
                QuoteStyle::None => {
                    if current == '\\' {
                        if let Some((next_relative, next)) = iter.peek().copied() {
                            iter.next();
                            end = start + next_relative + next.len_utf8();
                            text.push(next);
                        } else {
                            text.push(current);
                        }
                    } else if current == '\'' {
                        active_quote = QuoteStyle::Single;
                    } else if current == '"' {
                        active_quote = QuoteStyle::Double;
                    } else {
                        text.push(current);
                    }
                }
            }
        }

        tokens.push(Token {
            text,
            start: replacement_start,
            end,
            quote,
        });
    }

    let slice = &buffer[start..cursor];
    if slice.is_empty() || slice.chars().last().is_some_and(char::is_whitespace) {
        tokens.push(Token {
            text: String::new(),
            start: cursor,
            end: cursor,
            quote: QuoteStyle::None,
        });
    }

    tokens
}

pub(crate) fn quote_candidate(value: &str, quote: QuoteStyle) -> Option<String> {
    match quote {
        QuoteStyle::None => Some(escape_unquoted(value)),
        QuoteStyle::Double => {
            let mut escaped = String::with_capacity(value.len());
            for ch in value.chars() {
                if matches!(ch, '\\' | '"' | '$') || ch == char::from(96u8) {
                    escaped.push('\\');
                }
                escaped.push(ch);
            }
            Some(escaped)
        }
        QuoteStyle::Single => {
            if value.contains('\'') {
                None
            } else {
                Some(value.to_owned())
            }
        }
    }
}

fn escape_unquoted(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());

    for ch in value.chars() {
        if ch.is_whitespace()
            || ch == char::from(96u8)
            || matches!(
                ch,
                '\\' | '\'' | '"' | '$' | '!' | '&' | ';' | '|' | '<' | '>' | '('
                    | ')' | '[' | ']' | '{' | '}' | '*' | '?' | '#'
            )
        {
            escaped.push('\\');
        }
        escaped.push(ch);
    }

    escaped
}

#[cfg(test)]
mod tests {
    use super::{
        active_segment_start, active_segment_tokens, quote_candidate, tokens_before_cursor,
        QuoteStyle,
    };

    #[test]
    fn keeps_spaces_inside_open_double_quotes() {
        let input = "cat \"My Doc";
        let tokens = tokens_before_cursor(input, input.len());
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[1].text, "My Doc");
        assert_eq!(tokens[1].quote, QuoteStyle::Double);
        assert_eq!(&input[tokens[1].start..tokens[1].end], "My Doc");
    }

    #[test]
    fn unescapes_backslash_space() {
        let input = "cat My\\ Doc";
        let tokens = tokens_before_cursor(input, input.len());
        assert_eq!(tokens[1].text, "My Doc");
    }

    #[test]
    fn adds_empty_token_after_space() {
        let input = "git checkout ";
        let tokens = tokens_before_cursor(input, input.len());
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[2].text, "");
    }

    #[test]
    fn double_quote_candidate_keeps_spaces_unescaped() {
        assert_eq!(
            quote_candidate("My Documents/file.txt", QuoteStyle::Double).as_deref(),
            Some("My Documents/file.txt")
        );
    }

    #[test]
    fn unquoted_candidate_escapes_spaces() {
        assert_eq!(
            quote_candidate("My Documents/file.txt", QuoteStyle::None).as_deref(),
            Some("My\\ Documents/file.txt")
        );
    }

    #[test]
    fn pipeline_uses_only_active_segment() {
        let input = "cat file | gre";
        let tokens = active_segment_tokens(input, input.len());
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].text, "gre");
        assert_eq!(&input[tokens[0].start..tokens[0].end], "gre");
    }

    #[test]
    fn and_separator_uses_only_right_segment() {
        let input = "git status && dock";
        let tokens = active_segment_tokens(input, input.len());
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].text, "dock");
    }

    #[test]
    fn semicolon_separator_uses_only_right_segment() {
        let input = "pwd; cd ~/Doc";
        let tokens = active_segment_tokens(input, input.len());
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].text, "cd");
        assert_eq!(tokens[1].text, "~/Doc");
    }

    #[test]
    fn separator_inside_quotes_does_not_split() {
        let input = "grep \"a|b\" fi";
        let tokens = active_segment_tokens(input, input.len());
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].text, "grep");
        assert_eq!(tokens[1].text, "a|b");
        assert_eq!(tokens[2].text, "fi");
    }

    #[test]
    fn escaped_separator_does_not_split() {
        let input = "echo a\\|b";
        assert_eq!(active_segment_start(input, input.len()), 0);
        let tokens = active_segment_tokens(input, input.len());
        assert_eq!(tokens[1].text, "a|b");
    }

    #[test]
    fn background_separator_starts_new_segment() {
        let input = "sleep 1 & sys";
        let tokens = active_segment_tokens(input, input.len());
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].text, "sys");
    }
}
