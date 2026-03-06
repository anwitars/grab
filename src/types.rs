/// Result that can return any error type.
pub type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;
