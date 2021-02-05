const BASE64_ALPHABET: &'static [u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Adapted from [vlq](https://github.com/Rich-Harris/vlq/blob/822db3f22bf09148b84e8ef58878d11f3bcd543e/src/vlq.ts#L63)
fn vlq_encode_integer_to_buffer(buf: &mut String, mut value: isize) {
    if value.is_negative() {
        value = (-value << 1) | 1;
    } else {
        value <<= 1;
    };

    loop {
        let mut clamped = value & 31;
		value >>= 5;
		if value > 0 {
			clamped |= 32;
		}
		buf.push(BASE64_ALPHABET[clamped as usize] as char);
        if value <= 0 {
            break;
        }
    }
}

/// Struct for building a [source map (v3)](https://sourcemaps.info/spec.html)
pub struct SourceMap {
    buf: String,
    line: u8,
    column: u8,
    // The last a mapping was added to. Used to decide whether to add segment separator ','
    last_line: u8,
    sources: Vec<(String, Option<String>)>, //
}

impl SourceMap {
    pub fn new() -> Self {
        SourceMap {
            buf: String::new(),
            line: 0, 
            column: 0,
            // should be -1 but usize, if 0 or any other may confuse that initial starts of on A line
            last_line: u8::MAX, 
            sources: Vec::new(),
        }
    }

    /// Original line and original column are one indexed
    pub fn add_mapping(&mut self, original_line: usize, original_column: usize) {
        if self.last_line == self.line {
            self.buf.push(',');
        }
        self.last_line = self.line;
        
        vlq_encode_integer_to_buffer(&mut self.buf, self.column.into());
        self.buf.push('A');
        vlq_encode_integer_to_buffer(&mut self.buf, original_line as isize - 1);
        vlq_encode_integer_to_buffer(&mut self.buf, original_column as isize  - 1);
    }

    pub fn add_new_line(&mut self) {
        self.line += 1;
        self.buf.push(';');
        self.column = 0;
    }

    pub fn add_to_column(&mut self, length: usize) {
        self.column += length as u8;
    }

    /// TODO kinda temp
    pub fn add_source(&mut self, name: String, content: Option<String>) {
        self.sources.push((name, content))
    }

    pub fn to_string(self) -> String {
        let mut source_names = String::new();
        let mut source_contents = String::new();
        for (idx, (source_name, source_content)) in self.sources.iter().enumerate() {
            source_names.push('"');
            source_names.push_str(source_name);
            source_names.push('"');
            source_contents.push('"');
            if let Some(content) = &source_content {
                source_contents.push_str(&content.replace('\n', "\\n")); // TODO \r ..?
            }
            source_contents.push('"');
            if idx < self.sources.len() - 1 {
                source_names.push(',');
                source_contents.push(',');
            }
        }
        format!(
            r#"{{"version":3,"sourceRoot":"","sources":[{}],"sourcesContent":[{}],"names":[],"mappings":"{}"}}"#,
            source_names,
            source_contents,
            self.buf
        )
    }
}

#[cfg(test)]
mod source_map_tests {
    use super::vlq_encode_integer_to_buffer;

    fn vlq_encode_integer(value: isize) -> String {
        let mut buf = String::new();
        vlq_encode_integer_to_buffer(&mut buf, value);
        buf
    }

    #[test]
    fn vlq_encoder() {
        assert_eq!(vlq_encode_integer(0), "A");
        assert_eq!(vlq_encode_integer(1), "C");
        assert_eq!(vlq_encode_integer(-1), "D");
        assert_eq!(vlq_encode_integer(123), "2H");
        assert_eq!(vlq_encode_integer(123456789), "qxmvrH");
    }
}