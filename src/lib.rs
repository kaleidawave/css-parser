//! # CSS parser
//!
//! Simple CSS parser and "renderer"

mod lexer;
use tokenizer_lib::{StreamedTokenChannel, StaticTokenChannel, TokenReader, Token};
pub use lexer::{lex_source, CSSToken};
use std::thread;

/// Position of token, line_start, column_start, line_end, column_end, 
/// could do filename..? pub Arc<String>
#[derive(Debug)]
pub struct Span(pub usize, pub usize, pub usize, pub usize);

#[derive(Debug)]
pub struct ParseError {
    reason: String,
    position: Span
}

impl From<Option<(CSSToken, Token<CSSToken, Span>)>> for ParseError {
    fn from(opt: Option<(CSSToken, Token<CSSToken, Span>)>) -> Self {
        if let Some((expected_type, invalid_token)) = opt {
            Self {
                reason: format!("Expected '{:?}' found '{:?}'", expected_type, invalid_token.0),
                position: invalid_token.1
            }
        } else {
            unreachable!()
        }
    }
}

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
            let (mut sender, mut reader) = StreamedTokenChannel::new();
            let parse_source_thread =
                thread::spawn(move || lexer::lex_source(&string, &mut sender));

            let this = Self::from_reader(&mut reader).and_then(|val| {
                // Checks script parsing did not throw
                parse_source_thread.join().unwrap().and(Ok(val))
            });
            reader.expect_next(CSSToken::EOS)?;
            this
        } else {
            let mut reader = StaticTokenChannel::new();
            lexer::lex_source(&string, &mut reader)?;
            let this = Self::from_reader(&mut reader);
            reader.expect_next(CSSToken::EOS)?;
            this
        }
    }

    /// Parses structure from string
    #[cfg(target_arch = "wasm32")]
    fn from_string(string: String) -> Result<Self, ParseError> {
        let mut reader = StaticTokenChannel::new();
        lexer::lex_source(&string, &mut reader)?;
        let this = Self::from_reader(&mut reader);
        reader.expect_next(CSSToken::EOS)?;
        this
    }

    /// Returns position of node as span **as it was parsed**. May be invalid or none after mutation
    fn get_position(&self) -> Option<&Span>;

    fn from_reader(reader: &mut impl TokenReader<CSSToken, Span>) -> Result<Self, ParseError>;

    fn to_string_from_buffer(&self, buf: &mut String, settings: &ToStringSettings, depth: u8);

    /// Returns structure as valid string
    fn to_string(&self, settings: &ToStringSettings) -> String {
        let mut buf = String::new();
        self.to_string_from_buffer(&mut buf, settings, 0);
        buf
    }
}

/// A css rule with a selector and collection of declarations
#[derive(Debug)]
pub struct Rule {
    selector: Selector,
    declarations: Vec<(String, String)>,
    position: Option<Span>
}

impl ASTNode for Rule {
    fn from_reader(reader: &mut impl TokenReader<CSSToken, Span>) -> Result<Self, ParseError> {
        let selector = Selector::from_reader(reader)?;
        let Span(line_start, column_start, ..) = selector.get_position().unwrap();
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
            if CSSToken::SemiColon != reader.next().unwrap().0 {
                break;
            }
        }
        let Span(.., line_end, column_end) = reader.expect_next(CSSToken::CloseCurly)?;
        Ok(Self {
            position: Some(Span(*line_start, *column_start, line_end, column_end)),
            selector,
            declarations,
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
        self.position.as_ref()
    }
}

/// [A css selector](https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_Selectors)
#[derive(Debug)]
pub enum Selector {
    TagName(String, Option<Span>),
}

impl ASTNode for Selector {
    fn from_reader(reader: &mut impl TokenReader<CSSToken, Span>) -> Result<Self, ParseError> {
        match reader.next().unwrap() {
            Token(CSSToken::Ident(name), pos) => Ok(Self::TagName(name, Some(pos))),
            Token(token, _) => panic!("Invalid token {:?}", token)
        }
    }

    fn to_string_from_buffer(&self, buf: &mut String, _settings: &ToStringSettings, _depth: u8) {
        match self {
            Self::TagName(name, _pos) => {
                buf.push_str(&name);
            }
        }
    }

    fn get_position(&self) -> Option<&Span> {
        match self {
            Self::TagName(_, position) => position.as_ref()
        }
    }
}

/// A StyleSheet with a collection of rules
#[derive(Debug)]
pub struct StyleSheet {
    pub rules: Vec<Rule>
}

impl ASTNode for StyleSheet {
    fn from_reader(reader: &mut impl TokenReader<CSSToken, Span>) -> Result<Self, ParseError> {
        let mut rules: Vec<Rule> = Vec::new();
        while reader.peek().is_some() && reader.peek().unwrap().0 != CSSToken::EOS {
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
        let style_sheet = StyleSheet::from_string(include_str!("../examples/example1.css").to_owned()).unwrap();
        assert_eq!(style_sheet.rules.len(), 2);
    }

    #[test]
    fn style_sheet_to_string() {
        let source = include_str!("../examples/example1.css").to_owned();
        let style_sheet = StyleSheet::from_string(source.clone()).unwrap();
        assert_eq!(style_sheet.to_string(&ToStringSettings::default()), source.replace('\r', ""));
    }
}