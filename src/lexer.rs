use super::{ParseError, Span};
use tokenizer_lib::{Token, TokenSender};

#[derive(PartialEq, Eq, Debug)]
pub enum CSSToken {
    Ident(String),
    OpenCurly,
    CloseCurly,
    Colon,
    SemiColon,
    /// END of source
    EOS,
}

/// Lexes the source returning CSSToken sequence
pub fn lex_source(
    source: &String,
    sender: &mut impl TokenSender<CSSToken, Span>,
) -> Result<(), ParseError> {
    #[derive(PartialEq)]
    enum ParsingState {
        Ident,
        None,
    }

    let mut line_start = 1;
    let mut line_end = line_start;
    let mut column_start = 1;
    let mut column_end = column_start;
    let mut state = ParsingState::None;
    let mut last = 0;

    for (idx, chr) in source.char_indices() {
        if chr == '\n' {
            line_end += 1;
            column_end = 0;
        }

        macro_rules! set_state {
            ($s:expr) => {{
                last = idx;
                line_start = line_end;
                column_start = column_end;
                state = $s;
            }};
        }
        macro_rules! push_token {
            ($t:expr) => {{
                sender.push(Token(
                    $t,
                    Span(line_start, column_start, line_end, column_end),
                ));
            }};
        }

        match state {
            ParsingState::Ident => match chr {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' => {}
                _ => {
                    push_token!(CSSToken::Ident(source[last..idx].to_owned()));
                    set_state!(ParsingState::None);
                }
            },
            ParsingState::None => {}
        }

        if state == ParsingState::None {
            match chr {
                'A'..='Z' | 'a'..='z' => set_state!(ParsingState::Ident),
                chr => {
                    if chr.is_whitespace() {
                        column_end += chr.len_utf16();
                        column_start = column_end;
                        continue;
                    }
                    let token = match chr {
                        '{' => CSSToken::OpenCurly,
                        '}' => CSSToken::CloseCurly,
                        ':' => CSSToken::Colon,
                        ';' => CSSToken::SemiColon,
                        chr => unimplemented!("Invalid character '{}'", chr),
                    };
                    column_end += chr.len_utf16();
                    push_token!(token);
                    continue;
                }
            }
        }

        if chr != '\n' {
            column_end += chr.len_utf16();
        }
    }

    sender.push(Token(
        CSSToken::EOS,
        Span(line_end, column_end, line_end, column_end),
    ));

    Ok(())
}
