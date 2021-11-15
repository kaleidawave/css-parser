use super::{token_as_ident, ASTNode, CSSToken, ParseError, Span, ToStringSettings};
use source_map::ToString;
use tokenizer_lib::{Token, TokenReader};

/// [A css selector](https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_Selectors)
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Selector {
    /// Can be '*' for universal
    tag_name: Option<String>,
    /// #...
    identifier: Option<String>,
    /// .x.y.z
    class_names: Option<Vec<String>>,
    /// div h1
    descendant: Option<Box<Selector>>,
    /// div > h1
    child: Option<Box<Selector>>,
    position: Option<Span>,
}

impl ASTNode for Selector {
    fn from_reader(reader: &mut impl TokenReader<CSSToken, Span>) -> Result<Self, ParseError> {
        let mut selector: Self = Self {
            tag_name: None,
            identifier: None,
            class_names: None,
            descendant: None,
            child: None,
            position: None,
        };
        for i in 0.. {
            // Handling "descendant" parsing by checking gap/space in tokens
            let Token(peek_token, peek_span) = reader.peek().unwrap();

            if matches!(peek_token, CSSToken::Comma) {
                return Ok(selector);
            }

            if i != 0
                && !matches!(peek_token, CSSToken::CloseAngle)
                && !selector
                    .position
                    .as_ref()
                    .unwrap()
                    .is_adjacent_to(peek_span)
            {
                let descendant = Self::from_reader(reader)?;
                selector.descendant = Some(Box::new(descendant));
                break;
            }
            match reader.next().unwrap() {
                Token(CSSToken::Ident(name), pos) => {
                    if let Some(_) = selector.tag_name.replace(name) {
                        return Err(ParseError {
                            reason: "Tag name specified twice".to_owned(),
                            position: pos,
                        });
                    }
                    selector.position = Some(pos);
                }
                Token(CSSToken::Asterisk, pos) => {
                    if let Some(_) = selector.tag_name.replace("*".to_owned()) {
                        return Err(ParseError {
                            reason: "Tag name specified twice".to_owned(),
                            position: pos,
                        });
                    }
                    selector.position = Some(pos);
                }
                Token(CSSToken::Dot, start_span) => {
                    let (class_name, end_span) = token_as_ident(reader.next().unwrap())?;
                    selector
                        .class_names
                        .get_or_insert_with(|| Vec::new())
                        .push(class_name);
                    if let Some(ref mut selector_position) = selector.position {
                        *selector_position = selector_position.union(&end_span);
                    } else {
                        selector.position = Some(start_span.union(&end_span));
                    }
                }
                Token(CSSToken::HashPrefixedValue(identifier), position) => {
                    if selector.identifier.replace(identifier).is_some() {
                        return Err(ParseError {
                            reason: "Cannot specify to id selectors".to_owned(),
                            position,
                        });
                    }
                    if let Some(ref mut selector_position) = selector.position {
                        *selector_position = selector_position.union(&position);
                    } else {
                        selector.position = Some(position);
                    }
                }
                Token(CSSToken::CloseAngle, position) => {
                    let child = Self::from_reader(reader)?;
                    if let Some(ref mut selector_position) = selector.position {
                        *selector_position = selector_position.union(&position);
                    } else {
                        return Err(ParseError {
                            reason: "Expected selector start, found '>'".to_owned(),
                            position,
                        });
                    }
                    selector.child = Some(Box::new(child));
                    break;
                }
                Token(token, position) => {
                    return Err(ParseError {
                        reason: format!("Expected valid selector found '{:?}'", token),
                        position,
                    });
                }
            }
            if matches!(
                reader.peek(),
                Some(Token(CSSToken::OpenCurly, _)) | Some(Token(CSSToken::EOS, _))
            ) {
                break;
            }
        }
        Ok(selector)
    }

    fn to_string_from_buffer(
        &self,
        buf: &mut impl ToString,
        settings: &ToStringSettings,
        depth: u8,
    ) {
        if let Some(ref pos) = self.position {
            buf.add_mapping(pos);
        }

        if let Some(name) = &self.tag_name {
            buf.push_str(name);
        }
        if let Some(id) = &self.identifier {
            buf.push('#');
            buf.push_str(id);
        }
        if let Some(class_names) = &self.class_names {
            for class_name in class_names.iter() {
                buf.push('.');
                buf.push_str(class_name);
            }
        }
        if let Some(descendant) = &self.descendant {
            buf.push(' ');
            descendant.to_string_from_buffer(buf, settings, depth);
        } else if let Some(child) = &self.child {
            if !settings.minify {
                buf.push(' ');
            }
            buf.push('>');
            if !settings.minify {
                buf.push(' ');
            }
            child.to_string_from_buffer(buf, settings, depth);
        }
    }

    fn get_position(&self) -> Option<&Span> {
        self.position.as_ref()
    }
}

impl Selector {
    /// Returns other nested under self
    pub fn nest_selector(&self, other: Self) -> Self {
        let mut new_selector = self.clone();
        // Walk down the new selector descendant and child branches until at end. Then set descendant value
        // on the tail. Uses raw pointers & unsafe due to issues with Rust borrow checker
        let mut tail: *mut Selector = &mut new_selector;
        loop {
            let cur = unsafe { &mut *tail };
            if let Some(child) = cur.descendant.as_mut().or(cur.child.as_mut()) {
                tail = &mut **child;
            } else {
                cur.descendant = Some(Box::new(other));
                break;
            }
        }
        new_selector
    }
}

#[cfg(test)]
mod selector_tests {
    use source_map::SourceId;

    use super::*;

    const NULL_SOURCE_ID: SourceId = SourceId::null();

    #[test]
    fn tag_name() {
        let selector = Selector::from_string("h1".to_owned(), NULL_SOURCE_ID, None).unwrap();
        assert_eq!(
            selector.tag_name,
            Some("h1".to_owned()),
            "Bad selector {:?}",
            selector
        );
    }

    #[test]
    fn id() {
        let selector = Selector::from_string("#button1".to_owned(), NULL_SOURCE_ID, None).unwrap();
        assert_eq!(
            selector.identifier,
            Some("button1".to_owned()),
            "Bad selector {:?}",
            selector
        );
    }

    #[test]
    fn class_name() {
        let selector1 =
            Selector::from_string(".container".to_owned(), NULL_SOURCE_ID, None).unwrap();
        assert_eq!(
            selector1.class_names.as_ref().unwrap()[0],
            "container".to_owned(),
            "Bad selector {:?}",
            selector1
        );
        let selector2 =
            Selector::from_string(".container.center-x".to_owned(), NULL_SOURCE_ID, None).unwrap();
        assert_eq!(
            selector2.class_names.as_ref().unwrap().len(),
            2,
            "Bad selector {:?}",
            selector2
        );
    }

    #[test]
    fn descendant() {
        let selector =
            Selector::from_string("div .button".to_owned(), NULL_SOURCE_ID, None).unwrap();
        assert_eq!(
            selector.tag_name,
            Some("div".to_owned()),
            "Bad selector {:?}",
            selector
        );
        let descendant_selector = *selector.descendant.unwrap();
        assert_eq!(
            descendant_selector.class_names.as_ref().unwrap()[0],
            "button".to_owned(),
            "Bad selector {:?}",
            descendant_selector
        );
    }

    #[test]
    fn child() {
        let selector = Selector::from_string("div > h1".to_owned(), NULL_SOURCE_ID, None).unwrap();
        assert_eq!(
            selector.tag_name,
            Some("div".to_owned()),
            "Bad selector {:?}",
            selector
        );
        let child_selector = *selector.child.unwrap();
        assert_eq!(
            child_selector.tag_name,
            Some("h1".to_owned()),
            "Bad selector {:?}",
            child_selector
        );
    }
}
