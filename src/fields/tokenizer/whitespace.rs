use std::io::{BufRead, BufReader, Read};

use super::FieldTokenizer;

pub struct WhitespaceFieldTokenizer<R: Read> {
    line_reader: LineReader<R>,
    buffer: Vec<u8>,
    fields: Vec<&'static [u8]>,
}

impl<R: Read> WhitespaceFieldTokenizer<R> {
    pub fn new(reader: R) -> Self {
        Self {
            line_reader: LineReader::new(reader),
            buffer: Vec::new(),
            fields: Vec::new(),
        }
    }
}

impl<R: Read> FieldTokenizer for WhitespaceFieldTokenizer<R> {
    fn next_record(&mut self) -> crate::types::AnyResult<bool> {
        self.buffer.clear();
        self.fields.clear();

        #[inline]
        fn may_add_field(fields: &mut Vec<&'static [u8]>, line: &[u8], start: usize, end: usize) {
            if start < end {
                let mut slice =
                    unsafe { std::mem::transmute::<&[u8], &'static [u8]>(&line[start..end]) };
                if slice.first() == Some(&b'"') && slice.last() == Some(&b'"') {
                    slice = &slice[1..slice.len() - 1];
                }
                fields.push(slice);
            }
        }

        if let Some(line) = self.line_reader.read_line()? {
            let mut inside_quotes = false;
            let mut start = 0;

            for (i, &b) in line.iter().enumerate() {
                if b == b'"' {
                    inside_quotes = !inside_quotes;
                } else if b.is_ascii_whitespace() && !inside_quotes {
                    may_add_field(&mut self.fields, line, start, i);
                    start = i + 1;
                }
            }

            may_add_field(&mut self.fields, line, start, line.len());

            Ok(true)
        } else {
            return Ok(false);
        }
    }

    fn fields(&self) -> &[&str] {
        unsafe { std::mem::transmute::<&[&[u8]], &[&str]>(&self.fields) }
    }
}

struct LineReader<R: Read> {
    reader: BufReader<R>,
    buffer: Vec<u8>,
}

impl<R: Read> LineReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: BufReader::with_capacity(64 * 1024, reader),
            buffer: Vec::new(),
        }
    }

    pub fn read_line(&mut self) -> crate::types::AnyResult<Option<&[u8]>> {
        self.buffer.clear();

        let mut inside_quotes = false;
        let mut total_bytes_read = 0;

        loop {
            let bytes_read = self.reader.read_until(b'\n', &mut self.buffer)?;
            if bytes_read == 0 {
                break;
            }

            total_bytes_read += bytes_read;

            for &b in &self.buffer[total_bytes_read - bytes_read..total_bytes_read] {
                if b == b'"' {
                    inside_quotes = !inside_quotes;
                }
            }

            if !inside_quotes {
                break;
            }
        }

        if self.buffer.is_empty() {
            Ok(None)
        } else {
            Ok(Some(&self.buffer))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_line() {
        let mut tokenizer = WhitespaceFieldTokenizer::new(
            r#"field1 field2 "field 3 with spaces" field4"#.as_bytes(),
        );

        assert!(tokenizer.next_record().unwrap());
        let fields = tokenizer.fields();

        assert_eq!(
            fields,
            ["field1", "field2", "field 3 with spaces", "field4"]
        );
    }
}
