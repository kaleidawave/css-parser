use crate::token_as_ident;

use super::{ASTNode, CSSToken, CSSValue, ParseError, Selector, ToStringSettings};
use source_map::{Span, ToString};
use tokenizer_lib::{Token, TokenReader};

/// A css rule with a selector and collection of declarations
#[derive(Debug)]
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub nested_rules: Option<Vec<Rule>>,
    pub declarations: Vec<(String, CSSValue)>,
    pub position: Option<Span>,
}

impl ASTNode for Rule {
    fn from_reader(reader: &mut impl TokenReader<CSSToken, Span>) -> Result<Self, ParseError> {
        let mut selectors = vec![Selector::from_reader(reader)?];
        while let Token(CSSToken::Comma, _) = reader.peek().unwrap() {
            reader.next();
            selectors.push(Selector::from_reader(reader)?);
        }
        let first_span = selectors.first().unwrap().get_position().unwrap();
        reader.expect_next(CSSToken::OpenCurly)?;

        // Parse declarations and nested rules
        let mut declarations: Vec<(String, CSSValue)> = Vec::new();
        let mut nested_rules: Option<Vec<Rule>> = None;
        while let Some(Token(token_type, _)) = reader.peek() {
            if token_type == &CSSToken::CloseCurly {
                break;
            }
            let mut is_rule: Option<bool> = None;
            reader.scan(|token, _| {
                match token {
                    CSSToken::Colon | CSSToken::CloseCurly => is_rule = Some(false),
                    CSSToken::OpenCurly => is_rule = Some(true),
                    _ => {}
                }
                is_rule.is_some()
            });

            if is_rule.unwrap_or_default() {
                nested_rules
                    .get_or_insert_with(|| Vec::new())
                    .push(Rule::from_reader(reader)?);
            } else {
                let (property_name, _) = token_as_ident(reader.next().unwrap())?;
                reader.expect_next(CSSToken::Colon)?;
                let value = CSSValue::from_reader(reader)?;
                declarations.push((property_name, value));
                if let Token(CSSToken::CloseCurly, last_span) = reader.next().unwrap() {
                    return Ok(Self {
                        position: Some(first_span.union(&last_span)),
                        selectors,
                        declarations,
                        nested_rules,
                    });
                }
            }
        }
        let last_span = reader.expect_next(CSSToken::CloseCurly)?;
        Ok(Self {
            position: Some(first_span.union(&last_span)),
            selectors,
            declarations,
            nested_rules,
        })
    }

    fn to_string_from_buffer(
        &self,
        buf: &mut impl ToString,
        settings: &ToStringSettings,
        depth: u8,
    ) {
        for (idx, selector) in self.selectors.iter().enumerate() {
            selector.to_string_from_buffer(buf, settings, depth);
            if idx + 1 < self.selectors.len() {
                if settings.minify {
                    buf.push(',');
                } else {
                    buf.push_str(", ");
                }
            }
        }
        if !settings.minify {
            buf.push(' ');
        }
        buf.push('{');
        for (idx, (name, value)) in self.declarations.iter().enumerate() {
            if !settings.minify {
                buf.push_new_line();
                buf.push_str(&settings.indent_with.repeat(depth as usize + 1));
            }
            buf.push_str(name);
            buf.push(':');

            if !settings.minify {
                buf.push(' ');
            }
            value.to_string_from_buffer(buf, settings, depth);
            buf.push(';');
            if !settings.minify && idx == self.declarations.len() - 1 {
                buf.push_new_line();
                buf.push_str(&settings.indent_with.repeat(depth as usize));
            }
        }
        buf.push('}');
    }

    fn get_position(&self) -> Option<&Span> {
        self.position.as_ref()
    }
}
