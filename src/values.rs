use super::{ASTNode, CSSToken, ParseError, Span, ToStringSettings, Token};
use source_map::ToString;
use tokenizer_lib::TokenReader;

#[derive(Debug, PartialEq, Eq)]
pub struct Number(pub String);

#[derive(Debug, PartialEq, Eq)]
pub enum CSSValue {
    Keyword(String),
    Function(String, Vec<CSSValue>),
    StringLiteral(String),
    Number(Number),
    NumberWithUnit(Number, String),
    Percentage(Number),
    Color(String),
    List(Vec<CSSValue>),
    CommaSeparatedList(Vec<CSSValue>),
}

impl ASTNode for CSSValue {
    fn from_reader(reader: &mut impl TokenReader<CSSToken, Span>) -> Result<Self, ParseError> {
        let value = Self::single_value_from_reader(reader)?;
        macro_rules! css_value_has_ended {
            () => {
                matches!(
                    reader.peek().unwrap().0,
                    CSSToken::EOS | CSSToken::SemiColon | CSSToken::CloseCurly
                )
            };
        }
        if !css_value_has_ended!() {
            let mut values: Vec<CSSValue> = vec![value];
            while !css_value_has_ended!() {
                values.push(Self::single_value_from_reader(reader)?);
            }
            Ok(CSSValue::List(values))
        } else {
            Ok(value)
        }
    }

    fn to_string_from_buffer(
        &self,
        buf: &mut impl ToString,
        settings: &ToStringSettings,
        depth: u8,
    ) {
        match self {
            Self::Keyword(keyword) => buf.push_str(&keyword),
            Self::Color(color) => {
                buf.push('#');
                buf.push_str(&color);
            }
            Self::StringLiteral(content) => {
                buf.push('"');
                buf.push_str(&content);
                buf.push('"');
            }
            Self::Percentage(percent) => {
                buf.push_str(&percent.0);
                buf.push('%');
            }
            Self::Number(value) => {
                buf.push_str(&value.0);
            }
            Self::NumberWithUnit(value, unit) => {
                buf.push_str(&value.0);
                buf.push_str(&unit);
            }
            Self::List(values) => {
                for (idx, value) in values.iter().enumerate() {
                    value.to_string_from_buffer(buf, settings, depth);
                    if idx + 1 < values.len() && !settings.minify {
                        buf.push(' ');
                    }
                }
            }
            Self::CommaSeparatedList(values) => {
                for (idx, value) in values.iter().enumerate() {
                    value.to_string_from_buffer(buf, settings, depth);
                    if idx + 1 < values.len() {
                        buf.push(',');
                        if !settings.minify {
                            buf.push(' ');
                        }
                    }
                }
            }
            Self::Function(func, arguments) => {
                buf.push_str(&func);
                buf.push('(');
                for (idx, value) in arguments.iter().enumerate() {
                    value.to_string_from_buffer(buf, settings, depth);
                    if idx + 1 < arguments.len() {
                        buf.push(',');
                        if !settings.minify {
                            buf.push(' ');
                        }
                    }
                }
                buf.push(')');
            }
        }
    }

    fn get_position(&self) -> Option<&Span> {
        unreachable!()
    }
}

impl CSSValue {
    fn single_value_from_reader(
        reader: &mut impl TokenReader<CSSToken, Span>,
    ) -> Result<Self, ParseError> {
        match reader.next().unwrap() {
            Token(CSSToken::Ident(ident), start_span) => {
                let Token(peek_type, peek_span) = reader.peek().unwrap();
                if *peek_type == CSSToken::OpenBracket && start_span.is_adjacent_to(peek_span) {
                    reader.next();
                    reader.expect_next(CSSToken::CloseBracket)?;
                    todo!("Functions")
                } else {
                    Ok(CSSValue::Keyword(ident))
                }
            }
            Token(CSSToken::HashPrefixedValue(color), _) => Ok(CSSValue::Color(color)),
            Token(CSSToken::Number(number), start_position) => {
                let Token(peek_type, peek_position) = reader.peek().unwrap();
                let number = Number(number);
                if start_position.is_adjacent_to(peek_position)
                    && !matches!(peek_type, CSSToken::EOS | CSSToken::SemiColon)
                {
                    match peek_type {
                        CSSToken::Percentage => {
                            reader.next();
                            Ok(CSSValue::Percentage(number))
                        }
                        CSSToken::Ident(_) => {
                            let unit = if let CSSToken::Ident(unit) = reader.next().unwrap().0 {
                                unit
                            } else {
                                unreachable!()
                            };
                            Ok(CSSValue::NumberWithUnit(number, unit))
                        }
                        _ => Ok(CSSValue::Number(number)),
                    }
                } else {
                    Ok(CSSValue::Number(number))
                }
            }
            Token(CSSToken::String(string), _) => Ok(CSSValue::StringLiteral(string)),
            Token(token, position) => Err(ParseError {
                reason: format!("Expected value, found {:?}", token),
                position,
            }),
        }
    }
}

#[cfg(test)]
mod css_values_test {
    use super::{ASTNode, CSSValue, Number};
    use source_map::SourceId;

    const NULL_SOURCE_ID: SourceId = SourceId::null();

    macro_rules! test_value {
        ($test_name:ident, $src:expr, $res:expr) => {
            #[test]
            fn $test_name() {
                assert_eq!(
                    CSSValue::from_string($src.to_owned(), NULL_SOURCE_ID, None).unwrap(),
                    $res
                );
            }
        };
    }

    test_value!(keyword, "block", CSSValue::Keyword("block".to_owned()));
    test_value!(color, "#00ff00", CSSValue::Color("00ff00".to_owned()));
    test_value!(number, "1", CSSValue::Number(Number("1".to_owned())));
    test_value!(
        number_decimal_shorthand,
        ".2",
        CSSValue::Number(Number(".2".to_owned()))
    );
    test_value!(
        percentage,
        "10%",
        CSSValue::Percentage(Number("10".to_owned()))
    );
    test_value!(
        number_with_unit,
        "10px",
        CSSValue::NumberWithUnit(Number("10".to_owned()), "px".to_owned())
    );
    test_value!(
        list,
        "2px solid #00ff00",
        CSSValue::List(vec![
            CSSValue::NumberWithUnit(Number("2".to_owned()), "px".to_owned()),
            CSSValue::Keyword("solid".to_owned()),
            CSSValue::Color("00ff00".to_owned())
        ])
    );
}
