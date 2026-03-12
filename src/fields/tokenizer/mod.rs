mod csv;
mod whitespace;

use crate::types::AnyResult;

pub use csv::CsvFieldTokenizer;
pub use whitespace::WhitespaceFieldTokenizer;

pub trait FieldTokenizer {
    fn next_record(&mut self) -> AnyResult<bool>;
    fn fields(&self) -> &[&str];
}
