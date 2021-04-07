use super::{token_as_ident, ASTNode, CSSToken, ParseError, Span, ToStringSettings, ToStringer};
use std::boxed::Box;
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
            let peek = reader.peek().unwrap();
            if i != 0
                && peek.0 != CSSToken::CloseAngle
                && !selector.position.as_ref().unwrap().is_adjacent(&peek.1)
            {
                let descendant = Self::from_reader(reader)?;
                selector.position.as_mut().unwrap().2 = descendant.get_position().unwrap().2;
                selector.position.as_mut().unwrap().3 = descendant.get_position().unwrap().3;
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
                Token(CSSToken::Dot, Span(ls, cs, _, _, _)) => {
                    let (name, Span(_, _, le, ce, id)) = token_as_ident(reader.next().unwrap())?;
                    selector
                        .class_names
                        .get_or_insert_with(|| Vec::new())
                        .push(name);
                    if let Some(Span(_, _, ref mut ole, ref mut oce, _)) = &mut selector.position {
                        *ole = le;
                        *oce = ce;
                    } else {
                        selector.position = Some(Span(ls, cs, le, ce, id));
                    }
                }
                Token(CSSToken::HashTag, Span(ls, cs, _, _, _)) => {
                    let (name, Span(_, _, le, ce, id)) = token_as_ident(reader.next().unwrap())?;
                    if let Some(_) = selector.identifier.replace(name) {
                        return Err(ParseError {
                            reason: "Cannot specify to id selectors".to_owned(),
                            position: Span(ls, cs, le, ce, id),
                        });
                    }
                    if let Some(Span(_, _, ref mut ole, ref mut oce, _)) = &mut selector.position {
                        *ole = le;
                        *oce = ce;
                    } else {
                        selector.position = Some(Span(ls, cs, le, ce, id));
                    }
                }
                Token(CSSToken::CloseAngle, position) => {
                    let child = Self::from_reader(reader)?;
                    if let Some(Span(_, _, ref mut ole, ref mut oce, _)) = &mut selector.position {
                        *ole = child.get_position().unwrap().2;
                        *oce = child.get_position().unwrap().3;
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
                        reason: format!("Expected valid selector '{:?}'", token),
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
        buf: &mut ToStringer<'_>,
        settings: &ToStringSettings,
        depth: u8,
    ) {
        if let Some(pos) = &self.position {
            buf.add_mapping(pos.0, pos.1, pos.4);
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
    use super::*;

    #[test]
    fn tag_name() {
        let selector = Selector::from_string("h1".to_owned()).unwrap();
        assert_eq!(selector.tag_name, Some("h1".to_owned()));
    }

    #[test]
    fn id() {
        let selector = Selector::from_string("#button1".to_owned()).unwrap();
        assert_eq!(selector.identifier, Some("button1".to_owned()));
    }

    #[test]
    fn class_name() {
        let selector1 = Selector::from_string(".container".to_owned()).unwrap();
        assert_eq!(selector1.class_names.unwrap()[0], "container".to_owned());
        let selector2 = Selector::from_string(".container.center-x".to_owned()).unwrap();
        assert_eq!(selector2.class_names.unwrap().len(), 2);
    }

    #[test]
    fn descendant() {
        let selector = Selector::from_string("div .button".to_owned()).unwrap();
        assert_eq!(selector.tag_name, Some("div".to_owned()));
        let descendant_selector = *selector.descendant.unwrap();
        assert_eq!(
            descendant_selector.class_names.unwrap()[0],
            "button".to_owned()
        );
    }

    #[test]
    fn child() {
        let selector = Selector::from_string("div > h1".to_owned()).unwrap();
        assert_eq!(selector.tag_name, Some("div".to_owned()));
        let child_selector = *selector.child.unwrap();
        assert_eq!(child_selector.tag_name, Some("h1".to_owned()));
    }
}
