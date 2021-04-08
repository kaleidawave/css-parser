use super::{ASTNode, CSSToken, CSSValue, ParseError, Selector, Span, Token, ToStringer, ToStringSettings};
use tokenizer_lib::TokenReader;

/// A css rule with a selector and collection of declarations
#[derive(Debug)]
pub struct Rule {
    pub selector: Selector,
    pub nested_rules: Option<Vec<Rule>>,
    pub declarations: Vec<(String, CSSValue)>,
    position: Option<Span>,
}

impl ASTNode for Rule {
    fn from_reader(reader: &mut impl TokenReader<CSSToken, Span>) -> Result<Self, ParseError> {
        let selector = Selector::from_reader(reader)?;
        let Span(line_start, column_start, ..) = selector.get_position().unwrap();
        reader.expect_next(CSSToken::OpenCurly)?;
        let mut declarations: Vec<(String, CSSValue)> = Vec::new();
        let mut nested_rules: Option<Vec<Rule>> = None;
        while let Some(Token(token_type, _)) = reader.peek() {
            if token_type == &CSSToken::CloseCurly {
                break;
            }
            let mut is_rule: Option<bool> = None;
            reader.scan(|token| {
                match token {
                    CSSToken::Colon => is_rule = Some(false),
                    CSSToken::OpenCurly => is_rule = Some(true),
                    _ => {}
                }
                is_rule.is_some()
            });

            if is_rule.unwrap_or(false) {
                nested_rules
                    .get_or_insert_with(|| Vec::new())
                    .push(Rule::from_reader(reader)?);
            } else {
                let property = if let Some(Token(CSSToken::Ident(name), _)) = reader.next() {
                    name
                } else {
                    panic!()
                };
                reader.expect_next(CSSToken::Colon)?;
                let value = CSSValue::from_reader(reader)?;
                declarations.push((property, value));
                if CSSToken::SemiColon != reader.next().unwrap().0 {
                    break;
                }
            }
        }
        let Span(.., line_end, column_end, id) = reader.expect_next(CSSToken::CloseCurly)?;
        Ok(Self {
            position: Some(Span(*line_start, *column_start, line_end, column_end, id)),
            selector,
            declarations,
            nested_rules,
        })
    }

    fn to_string_from_buffer(
        &self,
        buf: &mut ToStringer<'_>,
        settings: &ToStringSettings,
        depth: u8,
    ) {
        self.selector.to_string_from_buffer(buf, settings, depth);
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
