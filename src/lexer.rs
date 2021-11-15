use source_map::{SourceId, Span};
use tokenizer_lib::{Token, TokenSender};

use crate::ParseError;

#[derive(PartialEq, Eq, Debug)]
pub enum CSSToken {
    Ident(String),
    Comment(String),
    /// HashPrefixedValue. Is a separate member to prevent lexing #0f5421 as a number.
    /// e.g #my-idx, #ffffff
    HashPrefixedValue(String),
    /// e.g 42
    Number(String),
    /// e.g "SF Pro Display"
    String(String),
    OpenCurly,
    CloseCurly,
    OpenBracket,
    CloseBracket,
    Colon,
    SemiColon,
    Dot,
    CloseAngle,
    Comma,
    Asterisk,
    Percentage,
    /// END of source
    EOS,
}

/// Lexes the source returning CSSToken sequence
/// byte_offset marks spans
pub fn lex_source(
    source: &str,
    sender: &mut impl TokenSender<CSSToken, Span>,
    source_id: SourceId,
    start_offset: Option<usize>,
) -> Result<(), ParseError> {
    #[derive(PartialEq)]
    enum ParsingState {
        Ident,
        Number,
        /// Used to decide whether class identifier or number
        Dot,
        String {
            escaped: bool,
        },
        HashPrefixedValue,
        Comment {
            found_asterisk: bool,
        },
        None,
    }

    let mut state = ParsingState::None;

    // Used for getting string slices from source
    let mut start = 0;
    let start_offset = start_offset.unwrap_or_default();

    for (idx, chr) in source.char_indices() {
        macro_rules! set_state {
            ($s:expr) => {{
                start = idx;
                state = $s;
            }};
        }

        macro_rules! push_token {
            ($t:expr) => {{
                if !sender.push(Token($t, current_position!())) {
                    return Ok(());
                };
            }};
        }

        macro_rules! current_position {
            () => {
                Span {
                    start: start_offset + start,
                    end: idx,
                    source_id,
                }
            };
        }

        match state {
            ParsingState::Ident => match chr {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' => {}
                _ => {
                    push_token!(CSSToken::Ident(source[start..idx].to_owned()));
                    set_state!(ParsingState::None);
                }
            },
            ParsingState::HashPrefixedValue => match chr {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' => {}
                _ => {
                    push_token!(CSSToken::HashPrefixedValue(
                        source[(start + 1)..idx].to_owned()
                    ));
                    set_state!(ParsingState::None);
                }
            },
            ParsingState::Dot => {
                if matches!(chr, '0'..='9') {
                    state = ParsingState::Number;
                } else {
                    push_token!(CSSToken::Dot);
                    set_state!(ParsingState::Ident);
                }
            }
            ParsingState::Number => match chr {
                '0'..='9' | '.' => {}
                _ => {
                    push_token!(CSSToken::Number(source[start..idx].to_owned()));
                    set_state!(ParsingState::None);
                }
            },
            ParsingState::String { ref mut escaped } => match chr {
                '\\' => {
                    *escaped = true;
                }
                '"' if !*escaped => {
                    push_token!(CSSToken::String(source[(start + 1)..idx].to_owned()));
                    set_state!(ParsingState::None);
                    continue;
                }
                _ => *escaped = false,
            },
            ParsingState::Comment {
                ref mut found_asterisk,
            } => match chr {
                '/' if *found_asterisk => {
                    push_token!(CSSToken::Comment(source[(start + 2)..(idx - 1)].to_owned()));
                    set_state!(ParsingState::None);
                    continue;
                }
                chr => {
                    *found_asterisk = chr == '*';
                }
            },
            ParsingState::None => {}
        }

        if state == ParsingState::None {
            match chr {
                'A'..='Z' | 'a'..='z' => set_state!(ParsingState::Ident),
                '/' => set_state!(ParsingState::Comment {
                    found_asterisk: true
                }),
                '.' => set_state!(ParsingState::Dot),
                '"' => set_state!(ParsingState::String { escaped: false }),
                '#' => set_state!(ParsingState::HashPrefixedValue),
                '0'..='9' => set_state!(ParsingState::Number),
                chr if chr.is_whitespace() => {
                    continue;
                }
                chr => {
                    let token = match chr {
                        '{' => CSSToken::OpenCurly,
                        '}' => CSSToken::CloseCurly,
                        '(' => CSSToken::OpenBracket,
                        ')' => CSSToken::CloseBracket,
                        ':' => CSSToken::Colon,
                        ';' => CSSToken::SemiColon,
                        ',' => CSSToken::Comma,
                        '>' => CSSToken::CloseAngle,
                        '.' => CSSToken::Dot,
                        '*' => CSSToken::Asterisk,
                        '%' => CSSToken::Percentage,
                        chr => {
                            return Err(ParseError {
                                reason: format!("Invalid character '{}'", chr),
                                position: current_position!(),
                            })
                        }
                    };
                    start = idx;
                    push_token!(token);
                    continue;
                }
            }
        }
    }

    let end_of_source = source.len();

    match state {
        ParsingState::Ident => {
            sender.push(Token(
                CSSToken::Ident(source[start..].to_owned()),
                Span {
                    start,
                    end: end_of_source,
                    source_id,
                },
            ));
        }
        ParsingState::Number => {
            sender.push(Token(
                CSSToken::Number(source[start..].to_owned()),
                Span {
                    start,
                    end: end_of_source,
                    source_id,
                },
            ));
        }
        ParsingState::HashPrefixedValue => {
            sender.push(Token(
                CSSToken::HashPrefixedValue(source[(start + 1)..].to_owned()),
                Span {
                    start,
                    end: end_of_source,
                    source_id,
                },
            ));
        }
        ParsingState::Comment { .. } => {
            return Err(ParseError {
                reason: "Could not find end to comment".to_owned(),
                position: Span {
                    start,
                    end: end_of_source,
                    source_id,
                },
            })
        }
        ParsingState::String { .. } => {
            return Err(ParseError {
                reason: "Could not find end to string".to_owned(),
                position: Span {
                    start,
                    end: end_of_source,
                    source_id,
                },
            })
        }
        ParsingState::Dot => {
            return Err(ParseError {
                reason: "Found trailing \".\"".to_owned(),
                position: Span {
                    start,
                    end: end_of_source,
                    source_id,
                },
            })
        }
        ParsingState::None => {}
    }

    sender.push(Token(
        CSSToken::EOS,
        Span {
            start: end_of_source,
            end: end_of_source,
            source_id,
        },
    ));

    Ok(())
}
