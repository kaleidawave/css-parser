use tokenizer_lib::{ParseError, Span, Token, TokenSender};

#[derive(PartialEq, Eq, Debug)]
pub enum CSSToken {
    Ident(String),
    OpenCurly,
    CloseCurly,
    Colon,
    SemiColon,
}

/// Lexes the source returning CSSToken sequence
pub fn lex_source(
    source: &String,
    sender: &mut impl TokenSender<CSSToken>,
) -> Result<(), ParseError> {
    #[derive(PartialEq)]
    enum ParsingState {
        Ident,
        None,
    }

    let mut start = 0;

    let mut state = ParsingState::None;

    for (idx, chr) in source.char_indices() {
        macro_rules! set_state {
            ($s:expr) => {{
                start = idx;
                state = $s;
            }};
        }

        match state {
            ParsingState::Ident => match chr {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' => {}
                _ => {
                    sender.push(Token(
                        CSSToken::Ident(source[start..idx].to_owned()),
                        Span(start, idx),
                    ));
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
                        continue;
                    }
                    let token = match chr {
                        '{' => CSSToken::OpenCurly,
                        '}' => CSSToken::CloseCurly,
                        ':' => CSSToken::Colon,
                        ';' => CSSToken::SemiColon,
                        chr => unimplemented!("Invalid character '{}'", chr),
                    };
                    sender.push(Token(token, Span(idx, idx + 1)));
                }
            }
        }
    }

    Ok(())
}
