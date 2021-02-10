use super::{ParseError, Span};
use tokenizer_lib::{Token, TokenSender};

#[derive(PartialEq, Eq, Debug)]
pub enum CSSToken {
    Ident(String),
    Comment(String),
    OpenCurly,
    CloseCurly,
    Colon,
    SemiColon,
    Dot,
    HashTag,
    CloseAngle,
    Comma,
    Asterisk,
    /// END of source
    EOS,
}

/// Lexes the source returning CSSToken sequence
pub fn lex_source(
    source: &String,
    source_id: u8,    
    sender: &mut impl TokenSender<CSSToken, Span>,
) -> Result<(), ParseError> {
    #[derive(PartialEq)]
    enum ParsingState {
        Ident,
        Comment { found_asterisk: bool },
        None,
    }

    let mut state = ParsingState::None;

    // Used for the position of tokens
    let mut line_start = 1;
    let mut line_end = line_start;
    let mut column_start = 1;
    let mut column_end = column_start;

    macro_rules! current_position {
        () => {
            Span(line_end, column_end, line_end, column_end, source_id)
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
                    Span(line_start, column_start, line_end, column_end, source_id),
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
            },
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
                chr if chr.is_whitespace() => {
                    if chr == '\n' {
                        line_end += 1;
                        column_end = 1;
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
                        ':' => CSSToken::Colon,
                        ';' => CSSToken::SemiColon,
                        ',' => CSSToken::Comma,
                        '>' => CSSToken::CloseAngle,
                        '#' => CSSToken::HashTag,
                        '.' => CSSToken::Dot,
                        '*' => CSSToken::Asterisk,
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
            column_end = 1;
        } else {
            column_end += chr.len_utf16();
        }
    }

    if state == ParsingState::Ident {
        sender.push(Token(
            CSSToken::Ident(source[start..].to_owned()),
            Span(line_start, column_start, line_end, column_end, source_id),
        ));
    }

    sender.push(Token(
        CSSToken::EOS,
        Span(line_end, column_end, line_end, column_end, source_id),
    ));

    Ok(())
}
