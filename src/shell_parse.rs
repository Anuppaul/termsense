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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveContext {
    pub(crate) tokens: Vec<Token>,
    pub(crate) redirection_target: bool,
    pub(crate) suppress_suggestions: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameKind {
    DollarParen,
    Backtick,
    ProcessSub,
    GroupParen,
    BraceGroup,
}

#[derive(Debug, Clone, Copy)]
struct Frame {
    kind: FrameKind,
    start: usize,
    outer_quote: QuoteStyle,
}

#[derive(Debug, Clone)]
enum Lexeme {
    Word(Token),
    Redirection { needs_path: bool },
}

pub(crate) fn active_context(buffer: &str, cursor: usize) -> ActiveContext {
    if inside_open_heredoc(buffer, cursor) || inside_open_arithmetic(buffer, cursor) {
        return ActiveContext {
            tokens: Vec::new(),
            redirection_target: false,
            suppress_suggestions: true,
        };
    }

    let nested_start = active_nested_start(buffer, cursor);
    let segment_start = active_segment_start_from(buffer, nested_start, cursor);
    let lexemes = lex_range(buffer, segment_start, cursor);
    let trailing_space = buffer[..cursor]
        .chars()
        .last()
        .is_some_and(char::is_whitespace);

    let mut tokens = Vec::new();
    for lexeme in &lexemes {
        if let Lexeme::Word(token) = lexeme {
            tokens.push(token.clone());
        }
    }

    let redirection_target = match lexemes.as_slice() {
        [.., Lexeme::Redirection { needs_path: true }] => {
            tokens.push(Token {
                text: String::new(),
                start: cursor,
                end: cursor,
                quote: QuoteStyle::None,
            });
            true
        }
        [.., Lexeme::Redirection { needs_path: true }, Lexeme::Word(_)] if !trailing_space => true,
        _ => false,
    };

    if !redirection_target {
        let slice = &buffer[segment_start..cursor];
        if (slice.is_empty() || trailing_space)
            && !tokens
                .last()
                .is_some_and(|token| token.start == cursor && token.end == cursor)
        {
            tokens.push(Token {
                text: String::new(),
                start: cursor,
                end: cursor,
                quote: QuoteStyle::None,
            });
        }
    }

    ActiveContext {
        tokens,
        redirection_target,
        suppress_suggestions: false,
    }
}

pub(crate) fn active_segment_tokens(buffer: &str, cursor: usize) -> Vec<Token> {
    active_context(buffer, cursor).tokens
}

pub(crate) fn tokens_before_cursor(buffer: &str, cursor: usize) -> Vec<Token> {
    let lexemes = lex_range(buffer, 0, cursor);
    let mut tokens: Vec<Token> = lexemes
        .into_iter()
        .filter_map(|lexeme| match lexeme {
            Lexeme::Word(token) => Some(token),
            Lexeme::Redirection { .. } => None,
        })
        .collect();

    let before = &buffer[..cursor];
    if (before.is_empty() || before.chars().last().is_some_and(char::is_whitespace))
        && !tokens
            .last()
            .is_some_and(|token| token.start == cursor && token.end == cursor)
    {
        tokens.push(Token {
            text: String::new(),
            start: cursor,
            end: cursor,
            quote: QuoteStyle::None,
        });
    }

    tokens
}

pub(crate) fn active_segment_start(buffer: &str, cursor: usize) -> usize {
    let base = active_nested_start(buffer, cursor);
    active_segment_start_from(buffer, base, cursor)
}

fn active_nested_start(buffer: &str, cursor: usize) -> usize {
    let before = &buffer[..cursor];
    let mut frames: Vec<Frame> = Vec::new();
    let mut quote = QuoteStyle::None;
    let mut escaped = false;
    let mut iter = before.char_indices().peekable();

    while let Some((offset, ch)) = iter.next() {
        if escaped {
            escaped = false;
            continue;
        }

        if quote == QuoteStyle::Single {
            if ch == '\'' {
                quote = QuoteStyle::None;
            }
            continue;
        }

        if ch == '\\' {
            escaped = true;
            continue;
        }

        if ch == '\'' && quote != QuoteStyle::Double {
            quote = QuoteStyle::Single;
            continue;
        }

        if ch == '"' {
            quote = if quote == QuoteStyle::Double {
                QuoteStyle::None
            } else {
                QuoteStyle::Double
            };
            continue;
        }

        if ch == char::from(36u8) {
            if let Some((next_offset, '(')) = iter.peek().copied() {
                let arithmetic = before[next_offset + 1..].starts_with('(');
                if arithmetic {
                    continue;
                }

                iter.next();
                frames.push(Frame {
                    kind: FrameKind::DollarParen,
                    start: next_offset + 1,
                    outer_quote: quote,
                });
                quote = QuoteStyle::None;
                continue;
            }
        }

        if quote == QuoteStyle::None && matches!(ch, '<' | '>') {
            if let Some((next_offset, '(')) = iter.peek().copied() {
                iter.next();
                frames.push(Frame {
                    kind: FrameKind::ProcessSub,
                    start: next_offset + 1,
                    outer_quote: quote,
                });
                continue;
            }
        }

        if quote == QuoteStyle::None && ch == '(' {
            let previous = before[..offset]
                .chars()
                .rev()
                .find(|value| !value.is_whitespace());

            if previous.is_none()
                || previous.is_some_and(|value| matches!(value, ';' | '|' | '&' | '(' | '{'))
            {
                frames.push(Frame {
                    kind: FrameKind::GroupParen,
                    start: offset + ch.len_utf8(),
                    outer_quote: quote,
                });
                continue;
            }
        }

        if quote == QuoteStyle::None && ch == '{' {
            let previous = before[..offset]
                .chars()
                .rev()
                .find(|value| !value.is_whitespace());

            if previous.is_none()
                || previous.is_some_and(|value| matches!(value, ';' | '|' | '&' | '(' | '{'))
            {
                frames.push(Frame {
                    kind: FrameKind::BraceGroup,
                    start: offset + ch.len_utf8(),
                    outer_quote: quote,
                });
                continue;
            }
        }

        if ch == ')' && quote == QuoteStyle::None {
            if frames.last().is_some_and(|frame| {
                matches!(
                    frame.kind,
                    FrameKind::DollarParen | FrameKind::ProcessSub | FrameKind::GroupParen
                )
            }) {
                let frame = frames.pop().expect("frame exists");
                quote = frame.outer_quote;
                continue;
            }
        }

        if ch == '}' && quote == QuoteStyle::None {
            if frames
                .last()
                .is_some_and(|frame| frame.kind == FrameKind::BraceGroup)
            {
                let frame = frames.pop().expect("frame exists");
                quote = frame.outer_quote;
                continue;
            }
        }

        if ch == char::from(96u8) {
            if frames
                .last()
                .is_some_and(|frame| frame.kind == FrameKind::Backtick)
            {
                let frame = frames.pop().expect("frame exists");
                quote = frame.outer_quote;
            } else {
                frames.push(Frame {
                    kind: FrameKind::Backtick,
                    start: offset + ch.len_utf8(),
                    outer_quote: quote,
                });
                quote = QuoteStyle::None;
            }
        }
    }

    frames.last().map(|frame| frame.start).unwrap_or(0)
}

fn active_segment_start_from(buffer: &str, base: usize, cursor: usize) -> usize {
    let before = &buffer[base..cursor];
    let mut quote = QuoteStyle::None;
    let mut escaped = false;
    let mut last_boundary = base;
    let mut iter = before.char_indices().peekable();

    while let Some((relative_offset, ch)) = iter.next() {
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

        if ch == '&' {
            let absolute = base + relative_offset;
            let previous_is_redirect = absolute > base
                && buffer
                    .as_bytes()
                    .get(absolute - 1)
                    .is_some_and(|byte| *byte == b'>');

            if previous_is_redirect || iter.peek().is_some_and(|(_, next)| *next == '>') {
                continue;
            }
        }

        if matches!(ch, ';' | '|' | '&') {
            let mut boundary = base + relative_offset + ch.len_utf8();

            if matches!(ch, '|' | '&') {
                if let Some((next_relative, next)) = iter.peek().copied() {
                    if next == ch {
                        iter.next();
                        boundary = base + next_relative + next.len_utf8();
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

fn lex_range(buffer: &str, start: usize, cursor: usize) -> Vec<Lexeme> {
    let before = &buffer[start..cursor];
    let mut lexemes = Vec::new();
    let mut iter = before.char_indices().peekable();

    while let Some((relative_index, ch)) = iter.peek().copied() {
        if ch.is_whitespace() {
            iter.next();
            continue;
        }

        let remaining = &before[relative_index..];
        if let Some(length) = redirection_operator_len(remaining) {
            for _ in 0..length {
                iter.next();
            }
            lexemes.push(Lexeme::Redirection {
                needs_path: redirection_needs_path(&remaining[..length]),
            });
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
                if current == '&' {
                    let remaining = &before[relative_offset..];
                    if redirection_operator_len(remaining).is_some() {
                        break;
                    }
                } else {
                    break;
                }
            }

            if active_quote == QuoteStyle::None {
                let remaining = &before[relative_offset..];
                if redirection_operator_len(remaining).is_some() {
                    break;
                }
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

        lexemes.push(Lexeme::Word(Token {
            text,
            start: replacement_start,
            end,
            quote,
        }));
    }

    lexemes
}

fn redirection_operator_len(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    if bytes.is_empty() {
        return None;
    }

    if bytes.starts_with(b"&>>") {
        return Some(3);
    }
    if bytes.starts_with(b"&>") {
        return Some(2);
    }

    let mut index = 0;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }

    if index >= bytes.len() || !matches!(bytes[index], b'<' | b'>') {
        return None;
    }

    let operator = bytes[index];
    index += 1;

    if index < bytes.len() && bytes[index] == operator {
        index += 1;

        if operator == b'<' && index < bytes.len() && bytes[index] == b'<' {
            index += 1;
        }
    } else if index < bytes.len()
        && ((operator == b'>' && matches!(bytes[index], b'&' | b'|'))
            || (operator == b'<' && bytes[index] == b'>'))
    {
        index += 1;
    }

    Some(index)
}

fn redirection_needs_path(operator: &str) -> bool {
    if operator.contains(">&") {
        return false;
    }

    if operator.ends_with("<<") || operator.ends_with("<<<") {
        return false;
    }

    true
}

fn inside_open_arithmetic(buffer: &str, cursor: usize) -> bool {
    let before = &buffer[..cursor];
    let bytes = before.as_bytes();
    let mut index = 0;
    let mut depth = 0usize;
    let mut quote = QuoteStyle::None;
    let mut escaped = false;

    while index < bytes.len() {
        let ch = bytes[index] as char;

        if escaped {
            escaped = false;
            index += 1;
            continue;
        }

        if quote == QuoteStyle::Single {
            if ch == '\'' {
                quote = QuoteStyle::None;
            }
            index += 1;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            index += 1;
            continue;
        }

        if ch == '\'' && depth == 0 {
            quote = QuoteStyle::Single;
            index += 1;
            continue;
        }

        if index + 2 < bytes.len()
            && bytes[index] == b'$'
            && bytes[index + 1] == b'('
            && bytes[index + 2] == b'('
        {
            depth = depth.saturating_add(1);
            index += 3;
            continue;
        }

        if depth > 0 && index + 1 < bytes.len() && bytes[index] == b')' && bytes[index + 1] == b')'
        {
            depth = depth.saturating_sub(1);
            index += 2;
            continue;
        }

        index += 1;
    }

    depth > 0
}

fn inside_open_heredoc(buffer: &str, cursor: usize) -> bool {
    let before = &buffer[..cursor];
    let mut pending: Vec<(String, bool)> = Vec::new();

    for chunk in before.split_inclusive('\n') {
        let complete = chunk.ends_with('\n');
        let line = chunk.strip_suffix('\n').unwrap_or(chunk);

        if let Some((delimiter, strip_tabs)) = pending.first() {
            if !complete {
                return true;
            }

            let candidate = if *strip_tabs {
                line.trim_start_matches('\t')
            } else {
                line
            };

            if candidate == delimiter {
                pending.remove(0);
            }
            continue;
        }

        if !complete {
            break;
        }

        pending.extend(find_heredoc_delimiters(line));
    }

    !pending.is_empty()
}

fn find_heredoc_delimiters(line: &str) -> Vec<(String, bool)> {
    let bytes = line.as_bytes();
    let mut result = Vec::new();
    let mut index = 0;
    let mut quote = QuoteStyle::None;
    let mut escaped = false;

    while index < bytes.len() {
        let ch = bytes[index] as char;

        if escaped {
            escaped = false;
            index += 1;
            continue;
        }

        match quote {
            QuoteStyle::Single => {
                if ch == '\'' {
                    quote = QuoteStyle::None;
                }
                index += 1;
                continue;
            }
            QuoteStyle::Double => {
                if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    quote = QuoteStyle::None;
                }
                index += 1;
                continue;
            }
            QuoteStyle::None => {}
        }

        if ch == '\\' {
            escaped = true;
            index += 1;
            continue;
        }
        if ch == '\'' {
            quote = QuoteStyle::Single;
            index += 1;
            continue;
        }
        if ch == '"' {
            quote = QuoteStyle::Double;
            index += 1;
            continue;
        }

        if index + 1 < bytes.len() && bytes[index] == b'<' && bytes[index + 1] == b'<' {
            if index + 2 < bytes.len() && bytes[index + 2] == b'<' {
                index += 3;
                continue;
            }

            index += 2;
            let mut strip_tabs = false;

            if index < bytes.len() && bytes[index] == b'-' {
                strip_tabs = true;
                index += 1;
            }

            while index < bytes.len() && (bytes[index] as char).is_whitespace() {
                index += 1;
            }

            if index >= bytes.len() {
                break;
            }

            let mut delimiter = String::new();
            let delimiter_quote = if bytes[index] == b'\'' {
                index += 1;
                QuoteStyle::Single
            } else if bytes[index] == b'"' {
                index += 1;
                QuoteStyle::Double
            } else {
                QuoteStyle::None
            };

            while index < bytes.len() {
                let current = bytes[index] as char;

                if delimiter_quote == QuoteStyle::Single && current == '\'' {
                    index += 1;
                    break;
                }
                if delimiter_quote == QuoteStyle::Double && current == '"' {
                    index += 1;
                    break;
                }
                if delimiter_quote == QuoteStyle::None
                    && (current.is_whitespace() || matches!(current, ';' | '|' | '&'))
                {
                    break;
                }

                delimiter.push(current);
                index += 1;
            }

            if !delimiter.is_empty() {
                result.push((delimiter, strip_tabs));
            }
            continue;
        }

        index += 1;
    }

    result
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
                '\\' | '\''
                    | '"'
                    | '$'
                    | '!'
                    | '&'
                    | ';'
                    | '|'
                    | '<'
                    | '>'
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '*'
                    | '?'
                    | '#'
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
        active_context, active_segment_start, active_segment_tokens, quote_candidate,
        tokens_before_cursor, QuoteStyle,
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
        assert_eq!(tokens[2].start, input.len());
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
    }

    #[test]
    fn and_separator_uses_only_right_segment() {
        let input = "git status && dock";
        let tokens = active_segment_tokens(input, input.len());

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].text, "dock");
    }

    #[test]
    fn separator_inside_quotes_does_not_split() {
        let input = "grep \"a|b\" fi";
        let tokens = active_segment_tokens(input, input.len());

        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[1].text, "a|b");
        assert_eq!(tokens[2].text, "fi");
    }

    #[test]
    fn escaped_separator_does_not_split() {
        let input = "echo a\\|b";

        assert_eq!(active_segment_start(input, input.len()), 0);
        assert_eq!(active_segment_tokens(input, input.len())[1].text, "a|b");
    }

    #[test]
    fn output_redirection_marks_path_target() {
        let input = "echo hi > lo";
        let context = active_context(input, input.len());

        assert!(context.redirection_target);
        assert_eq!(context.tokens.last().expect("target").text, "lo");
    }

    #[test]
    fn fd_append_redirection_marks_path_target() {
        let input = "cmd 2>> lo";
        let context = active_context(input, input.len());

        assert!(context.redirection_target);
        assert_eq!(context.tokens.last().expect("target").text, "lo");
    }

    #[test]
    fn descriptor_duplication_is_not_path_target() {
        let input = "cmd 2>&1";
        let context = active_context(input, input.len());

        assert!(!context.redirection_target);
    }

    #[test]
    fn dollar_paren_routes_to_inner_command() {
        let input = "echo $(git che";
        let tokens = active_segment_tokens(input, input.len());

        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].text, "git");
        assert_eq!(tokens[1].text, "che");
    }

    #[test]
    fn nested_dollar_paren_uses_innermost_command() {
        let input = "echo $(printf %s $(git che";
        let tokens = active_segment_tokens(input, input.len());

        assert_eq!(tokens[0].text, "git");
    }

    #[test]
    fn backticks_route_to_inner_command() {
        let input = format!("echo {}git che", char::from(96u8));
        let tokens = active_segment_tokens(&input, input.len());

        assert_eq!(tokens[0].text, "git");
        assert_eq!(tokens[1].text, "che");
    }

    #[test]
    fn process_substitution_routes_to_inner_command() {
        let input = "diff <(git che";
        let tokens = active_segment_tokens(input, input.len());

        assert_eq!(tokens[0].text, "git");
        assert_eq!(tokens[1].text, "che");
    }

    #[test]
    fn output_process_substitution_routes_to_inner_command() {
        let input = "tee >(grep --r";
        let tokens = active_segment_tokens(input, input.len());

        assert_eq!(tokens[0].text, "grep");
        assert_eq!(tokens[1].text, "--r");
    }

    #[test]
    fn paren_group_routes_to_inner_command() {
        let input = "( git che";
        let tokens = active_segment_tokens(input, input.len());

        assert_eq!(tokens[0].text, "git");
        assert_eq!(tokens[1].text, "che");
    }

    #[test]
    fn brace_group_routes_to_inner_command() {
        let input = "{ docker lo";
        let tokens = active_segment_tokens(input, input.len());

        assert_eq!(tokens[0].text, "docker");
        assert_eq!(tokens[1].text, "lo");
    }

    #[test]
    fn nested_process_substitution_uses_innermost_frame() {
        let input = "diff <(cat <(git che";
        let tokens = active_segment_tokens(input, input.len());

        assert_eq!(tokens[0].text, "git");
    }

    #[test]
    fn open_arithmetic_expansion_suppresses_command_suggestions() {
        let input = "echo $((1 + 2";
        let context = active_context(input, input.len());

        assert!(context.suppress_suggestions);
        assert!(context.tokens.is_empty());
    }

    #[test]
    fn closed_arithmetic_expansion_restores_outer_context() {
        let input = "echo $((1 + 2)) && git che";
        let context = active_context(input, input.len());

        assert!(!context.suppress_suggestions);
        assert_eq!(context.tokens[0].text, "git");
        assert_eq!(context.tokens[1].text, "che");
    }

    #[test]
    fn heredoc_body_suppresses_suggestions() {
        let input = "cat <<EOF\nhello wor";
        let context = active_context(input, input.len());

        assert!(context.suppress_suggestions);
        assert!(context.tokens.is_empty());
    }

    #[test]
    fn heredoc_declaration_line_does_not_suppress() {
        let input = "cat <<EOF";
        let context = active_context(input, input.len());

        assert!(!context.suppress_suggestions);
    }

    #[test]
    fn closed_heredoc_restores_command_context() {
        let input = "cat <<EOF\nhello\nEOF\ngit che";
        let context = active_context(input, input.len());

        assert!(!context.suppress_suggestions);
        assert_eq!(context.tokens[0].text, "git");
        assert_eq!(context.tokens[1].text, "che");
    }

    #[test]
    fn tab_stripping_heredoc_closes_on_tabbed_delimiter() {
        let input = "cat <<-EOF\n\tbody\n\tEOF\ngit che";
        let context = active_context(input, input.len());

        assert!(!context.suppress_suggestions);
        assert_eq!(context.tokens[0].text, "git");
    }
}
