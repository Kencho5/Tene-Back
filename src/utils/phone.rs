use crate::error::{AppError, Result};

pub fn normalize_phone(input: &str) -> Result<String> {
    let digits: String = input.chars().filter(|c| c.is_ascii_digit()).collect();
    let digits = digits.strip_prefix("00").unwrap_or(&digits);

    let normalized = match digits.len() {
        9 => format!("995{digits}"),
        12 if digits.starts_with("995") => digits.to_string(),
        _ => String::new(),
    };

    if normalized.len() != 12 || !normalized[3..].starts_with('5') {
        return Err(AppError::BadRequest(
            "არასწორი მობილურის ნომერი".to_string(),
        ));
    }

    Ok(normalized)
}
