use super::{ParseError, SourceId, Span};
use tokenizer_lib::{Token, TokenSender};

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

const LINE_START: usize = 1;
const COLUMN_START: usize = 0;

/// Lexes the source returning CSSToken sequence
pub fn lex_source(
    source: &String,
    source_id: SourceId,
    sender: &mut impl TokenSender<CSSToken, Span>,
) -> Result<(), ParseError> {
    #[derive(PartialEq)]
    enum ParsingState {
        Ident,
        Number,
        String { escaped: bool },
        HashPrefixedValue,
        Comment { found_asterisk: bool },
        None,
    }

    let mut state = ParsingState::None;

    // Used for the position of tokens. Line is one based, column is 0 based
    let mut line_start = LINE_START;
    let mut line_end = line_start;
    let mut column_start = COLUMN_START;
    let mut column_end = column_start;

    macro_rules! current_position {
        () => {
            Span(line_start, column_start, line_end, column_end, source_id)
        };
    }

    // Used for getting string slices from source
    let mut start = 0;

    for (idx, chr) in source.char_indices() {
        macro_rules! set_state {
            ($s:expr) => {{
                start = idx;
                line_start = line_end;
                column_start = column_end;
                state = $s;
            }};
        }

        macro_rules! push_token {
            ($t:expr) => {{
                sender.push(Token(
                    $t,
                    current_position!(),
                ));
            }};
        }

        match state {
            ParsingState::Ident => match chr {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' => {}
                _ => {
                    push_token!(CSSToken::Ident(source[start..idx].to_owned()));
                    set_state!(ParsingState::None);
                }
            }
            ParsingState::HashPrefixedValue => match chr {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' => {}
                _ => {
                    push_token!(CSSToken::HashPrefixedValue(source[(start+1)..idx].to_owned()));
                    set_state!(ParsingState::None);
                }
            }
            ParsingState::Number => match chr {
                '0'..='9' | '.' => {}
                _ => {
                    push_token!(CSSToken::Number(source[start..idx].to_owned()));
                    set_state!(ParsingState::None);
                }
            }
            ParsingState::String { ref mut escaped } => match chr {
                '\\' => { 
                    *escaped = true;
                }
                '"' if !*escaped => {
                    push_token!(CSSToken::String(source[start..idx].to_owned()));
                    set_state!(ParsingState::None);
                    continue;
                }
                _ => { *escaped = false }
            }
            ParsingState::Comment {
                ref mut found_asterisk,
            } => match chr {
                '/' if *found_asterisk => {
                    column_end += 1;
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
                '"' => set_state!(ParsingState::String { escaped: false} ),
                '#' => set_state!(ParsingState::HashPrefixedValue),
                '0'..='9' => set_state!(ParsingState::Number),
                chr if chr.is_whitespace() => {
                    if chr == '\n' {
                        line_end += 1;
                        column_end = COLUMN_START;
                    } else {
                        column_end += chr.len_utf16();
                        column_start = column_end;
                    }
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
                    // TODO this handles that this is not a state:
                    line_start = line_end;
                    column_start = column_end;
                    column_end += chr.len_utf16();
                    push_token!(token);
                    continue;
                }
            }
        }

        if chr == '\n' {
            line_end += 1;
            column_end = COLUMN_START;
        } else {
            column_end += chr.len_utf16();
        }
    }

    match state {
        ParsingState::Ident => {
            sender.push(Token(
                CSSToken::Ident(source[start..].to_owned()),
                current_position!(),
            ));
        }
        ParsingState::Number => {
            sender.push(Token(
                CSSToken::Number(source[start..].to_owned()),
                current_position!(),
            ));
        }
        ParsingState::HashPrefixedValue => {
            sender.push(Token(
                CSSToken::HashPrefixedValue(source[(start+1)..].to_owned()),
                current_position!(),
            ));
        }
        ParsingState::Comment { .. } => {
            return Err(ParseError {
                reason: "Could not find end to comment".to_owned(),
                position: current_position!()
            })
        }
        ParsingState::String { .. } => {
            return Err(ParseError {
                reason: "Could not find end to string".to_owned(),
                position: current_position!()
            })
        }
        ParsingState::None => {}
    }

    sender.push(Token(
        CSSToken::EOS,
        Span(line_end, column_end, line_end, column_end, source_id),
    ));

    Ok(())
}
