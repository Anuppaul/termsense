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

pub(crate) fn tokens_before_cursor(buffer: &str, cursor: usize) -> Vec<Token> {
    let before = &buffer[..cursor];
    let mut tokens = Vec::new();
    let mut iter = before.char_indices().peekable();

    while let Some((index, ch)) = iter.peek().copied() {
        if ch.is_whitespace() {
            iter.next();
            continue;
        }

        let raw_start = index;
        let mut replacement_start = raw_start;
        let mut end = raw_start;
        let mut text = String::new();
        let mut quote = QuoteStyle::None;
        let mut active_quote = QuoteStyle::None;

        if ch == '\'' || ch == '"' {
            iter.next();
            replacement_start = index + ch.len_utf8();
            end = replacement_start;
            quote = if ch == '\'' {
                QuoteStyle::Single
            } else {
                QuoteStyle::Double
            };
            active_quote = quote;
        }

        while let Some((offset, current)) = iter.peek().copied() {
            if active_quote == QuoteStyle::None && current.is_whitespace() {
                break;
            }

            iter.next();
            end = offset + current.len_utf8();

            match active_quote {
                QuoteStyle::Single => {
                    if current == '\'' {
                        active_quote = QuoteStyle::None;
                        end = offset;
                        break;
                    }
                    text.push(current);
                }
                QuoteStyle::Double => {
                    if current == '"' {
                        active_quote = QuoteStyle::None;
                        end = offset;
                        break;
                    }
                    if current == '\\' {
                        if let Some((next_offset, next)) = iter.peek().copied() {
                            iter.next();
                            end = next_offset + next.len_utf8();
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
                        if let Some((next_offset, next)) = iter.peek().copied() {
                            iter.next();
                            end = next_offset + next.len_utf8();
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

    if before.is_empty() || before.chars().last().is_some_and(char::is_whitespace) {
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
    use super::{quote_candidate, tokens_before_cursor, QuoteStyle};

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
}
