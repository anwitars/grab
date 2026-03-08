/// If the expression evaluates to an error, report it and continue to the next iteration of the loop.
#[macro_export]
macro_rules! try_report {
    ($expr:expr, $line_number:expr) => {
        if let Err(e) = $expr {
            $crate::error::report_error(e.to_string(), $line_number);
            continue;
        }
    };
}

/// Reports an error with the line number to standard error.
pub fn report_error<E: AsRef<str>>(error: E, line_number: usize) {
    eprintln!("Error on line {}: {}", line_number, error.as_ref());
}
