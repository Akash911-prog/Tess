use crate::parser::constants::COMMAND_FILLERS;

pub fn normalize_text(text: &str) -> String {
    let mut normalized = text.trim().to_lowercase();
    normalized = normalized
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() || c == '\'' {
                c
            } else {
                ' '
            }
        })
        .collect();

    normalized = normalized.split_whitespace().collect::<Vec<_>>().join(" ");

    loop {
        let mut changed = false;
        for filler in COMMAND_FILLERS {
            if let Some(rest) = normalized.strip_prefix(filler) {
                if rest.is_empty() || rest.starts_with(' ') {
                    normalized = rest.trim_start().to_owned();
                    changed = true;
                    break;
                }
            }
        }
        if !changed {
            break;
        }
    }
    normalized
}
