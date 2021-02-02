//! # CSS parser
//!
//! Simple CSS parser and "renderer"

mod lexer;
use tokenizer_lib::{ParseError, get_streamed_token_channel, StaticTokenChannel, Span, TokenReader, Token};
use lexer::CSSToken;
use std::thread;

/// Settings for rendering ASTNodes to CSS
pub struct ToStringSettings {
    pub minify: bool,
    pub indent_with: String,
}

impl std::default::Default for ToStringSettings {
    fn default() -> Self {
        ToStringSettings {
            minify: false,
            indent_with: "    ".to_owned(),
        }
    }
}

impl ToStringSettings {
    /// Minified settings, ASTNode::to_string will not return whitespace
    pub fn minified() -> Self {
        ToStringSettings {
            minify: true,
            indent_with: "".to_owned(),
        }
    }
}

pub trait ASTNode: Sized {
    /// Parses structure from string
    #[cfg(not(target_arch = "wasm32"))]
    fn from_string(string: String) -> Result<Self, ParseError> {
        if string.len() > 2048 {
            let (mut sender, mut reader) = get_streamed_token_channel();
            let parse_source_thread =
                thread::spawn(move || lexer::lex_source(&string, &mut sender));

            Self::from_reader(&mut reader).and_then(|val| {
                // Checks script parsing did not throw
                parse_source_thread.join().unwrap().and(Ok(val))
            })
        } else {
            let mut reader = StaticTokenChannel::new();
            lexer::lex_source(&string, &mut reader)?;
            Self::from_reader(&mut reader)
        }
    }

    /// Parses structure from string
    #[cfg(target_arch = "wasm32")]
    fn from_string(string: String) -> Result<Self, ParseError> {
        let mut reader = StaticTokenChannel::new();
        lexer::lex_source(&string, &mut reader)?;
        Self::from_reader(&mut reader)
    }

    /// Returns position of node as span **as it was parsed**. May be invalid or none after mutation
    fn get_position(&self) -> Option<&Span>;

    fn from_reader(reader: &mut impl TokenReader<CSSToken>) -> Result<Self, ParseError>;

    fn to_string_from_buffer(&self, buf: &mut String, settings: &ToStringSettings, depth: u8);

    /// Returns structure as valid string
    fn to_string(&self, settings: &ToStringSettings) -> String {
        let mut buf = String::new();
        self.to_string_from_buffer(&mut buf, settings, 0);
        buf
    }
}

pub(crate) fn expected_token_err() -> ParseError {
    ParseError {
        reason: String::from("Expected token, found end"),
        position: None,
    }
}

/// A css rule with a selector and collection of declarations
#[derive(Debug)]
pub struct Rule {
    selector: Selector,
    declarations: Vec<(String, String)>
}

impl ASTNode for Rule {
    fn from_reader(reader: &mut impl TokenReader<CSSToken>) -> Result<Self, ParseError> {
        let selector = Selector::from_reader(reader)?;
        reader.expect_next(CSSToken::OpenCurly)?;
        let mut declarations: Vec<(String, String)> = Vec::new();
        while let Some(Token(token_type, _)) = reader.peek() {
            if token_type == &CSSToken::CloseCurly {
                break;
            }
            let property = if let Some(Token(CSSToken::Ident(name), _)) = reader.next() {
                name
            } else {
                panic!()
            };
            reader.expect_next(CSSToken::Colon)?;
            let value = if let Some(Token(CSSToken::Ident(name), _)) = reader.next() {
                name
            } else {
                panic!()
            };
            declarations.push((property, value));
            if CSSToken::SemiColon != reader.next().ok_or_else(expected_token_err)?.0 {
                break;
            }
        }
        reader.expect_next(CSSToken::CloseCurly)?;
        Ok(Self {
            selector,
            declarations
        })
    }

    fn to_string_from_buffer(&self, buf: &mut String, settings: &ToStringSettings, depth: u8) {
        self.selector.to_string_from_buffer(buf, settings, depth);
        if !settings.minify {
            buf.push(' ');
        }
        buf.push('{');
        for (idx, (name, value)) in self.declarations.iter().enumerate() {
            if !settings.minify {
                buf.push('\n');
                buf.push_str(&settings.indent_with.repeat(depth as usize + 1));
            }
            buf.push_str(name);
            buf.push(':');
            if !settings.minify {
                buf.push(' ');
            }
            buf.push_str(value);
            buf.push(';');
            if !settings.minify && idx == self.declarations.len() - 1 {
                buf.push('\n');
                buf.push_str(&settings.indent_with.repeat(depth as usize));
            }
        }
        buf.push('}');
    }

    fn get_position(&self) -> Option<&Span> {
        unimplemented!()
    }
}

/// [A css selector](https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_Selectors)
#[derive(Debug)]
pub enum Selector {
    TagName(String),
}

impl ASTNode for Selector {
    fn from_reader(reader: &mut impl TokenReader<CSSToken>) -> Result<Self, ParseError> {
        match reader.next().ok_or_else(expected_token_err)? {
            Token(CSSToken::Ident(name), _pos) => Ok(Self::TagName(name)),
            token => panic!("Invalid token {:#?}", token)
        }
    }

    fn to_string_from_buffer(&self, buf: &mut String, _settings: &ToStringSettings, _depth: u8) {
        match self {
            Self::TagName(name) => {
                buf.push_str(&name);
            }
        }
    }

    fn get_position(&self) -> Option<&Span> {
        unimplemented!()
    }
}

/// A Stylesheet with a collection of rules
#[derive(Debug)]
pub struct StyleSheet {
    pub rules: Vec<Rule>
}

impl ASTNode for StyleSheet {
    fn from_reader(reader: &mut impl TokenReader<CSSToken>) -> Result<Self, ParseError> {
        let mut rules: Vec<Rule> = Vec::new();
        while reader.peek().is_some() {
            rules.push(Rule::from_reader(reader)?);
        }
        Ok(Self { rules })
    }

    fn to_string_from_buffer(&self, buf: &mut String, settings: &ToStringSettings, _depth: u8) {
        for (idx, rule) in self.rules.iter().enumerate() {
            rule.to_string_from_buffer(buf, settings, 0);
            if !settings.minify && idx + 1 < self.rules.len() {
                buf.push_str("\n\n");
            }
        }
    }

    fn get_position(&self) -> Option<&Span> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_rules() {
        let stylesheet = StyleSheet::from_string(include_str!("../examples/example1.css").to_owned()).unwrap();
        assert_eq!(stylesheet.rules.len(), 2);
    }

    #[test]
    fn stylesheet_to_string() {
        let source = include_str!("../examples/example1.css").to_owned();
        let stylesheet = StyleSheet::from_string(source.clone()).unwrap();
        assert_eq!(stylesheet.to_string(&ToStringSettings::default()), source.replace('\r', ""));
    }
}