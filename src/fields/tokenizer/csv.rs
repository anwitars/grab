use crate::types::AnyResult;

use super::FieldTokenizer;
use std::io::Read;

pub struct CsvFieldTokenizer<R: Read> {
    reader: csv::Reader<R>,
    byte_record: csv::ByteRecord,
    fields: Vec<&'static [u8]>,
}

impl<R: Read> CsvFieldTokenizer<R> {
    pub fn new(reader: R, delimiter: u8) -> Self {
        let csv_reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .delimiter(delimiter)
            .buffer_capacity(64 * 1024)
            .from_reader(reader);

        Self {
            reader: csv_reader,
            byte_record: csv::ByteRecord::new(),
            fields: Vec::new(),
        }
    }
}

impl<R: Read> FieldTokenizer for CsvFieldTokenizer<R> {
    fn next_record(&mut self) -> AnyResult<bool> {
        self.fields.clear();
        if self.reader.read_byte_record(&mut self.byte_record)? {
            for field in self.byte_record.iter() {
                if std::str::from_utf8(field).is_err() {
                    return Err(format!("Invalid UTF-8 sequence in field: {:?}", field).into());
                }
                let slice = unsafe { std::mem::transmute::<&[u8], &'static [u8]>(field) };
                self.fields.push(slice);
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn fields(&self) -> &[&str] {
        unsafe { std::mem::transmute::<&[&[u8]], &[&str]>(&self.fields) }
    }
}
