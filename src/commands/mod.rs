pub mod change;
pub mod edit;
pub mod fix;
pub mod inspect;
pub mod rules;
pub mod schema;
pub mod validate;

use std::io::Read;

use crate::envelope::AppError;

/// Read one input: a path, or stdin for `-`.
pub fn read_input(arg: &str) -> Result<String, AppError> {
    let bytes = if arg == "-" {
        let mut buf = Vec::new();
        std::io::stdin().read_to_end(&mut buf).map_err(|e| AppError::input("stdin-not-readable", format!("Cannot read stdin: {e}.")))?;
        buf
    } else {
        std::fs::read(arg).map_err(|e| {
            let code = if e.kind() == std::io::ErrorKind::NotFound { "file-not-found" } else { "file-not-readable" };
            let what = if e.kind() == std::io::ErrorKind::NotFound { "File not found".to_string() } else { format!("Cannot read file ({e})") };
            AppError::input(code, format!("{what}: {arg}.")).path(arg)
        })?
    };
    String::from_utf8(bytes).map_err(|_| AppError::input("not-utf8", format!("{arg} is not UTF-8 text.")).path(arg))
}
