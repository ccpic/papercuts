use crate::ListItem;
use crate::error::{AppError, AppResult};
use std::collections::BTreeSet;

pub fn normalize_prefix(input: &str) -> AppResult<String> {
    let prefix = input
        .get(..3)
        .filter(|prefix| prefix.eq_ignore_ascii_case("pc_"))
        .map_or(input, |_| &input[3..]);
    if prefix.len() < 4 || !prefix.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AppError::invalid_argument(
            format!("invalid papercut ID prefix '{input}'"),
            "Use `papercuts list --status all` and pass at least 4 hexadecimal digits, with optional pc_ prefix.",
        ));
    }
    Ok(prefix.to_ascii_lowercase())
}

pub fn match_id(prefix: &str, items: &[ListItem]) -> AppResult<String> {
    let mut candidates: Vec<_> = items
        .iter()
        .map(|item| item.cut.id.clone())
        .filter(|id| {
            id.get(..3)
                .filter(|id_prefix| id_prefix.eq_ignore_ascii_case("pc_"))
                .and_then(|_| id.get(3..))
                .is_some_and(|hex| hex.to_ascii_lowercase().starts_with(prefix))
        })
        .collect();
    candidates.sort();
    match candidates.as_slice() {
        [] => Err(AppError::not_found(
            format!("no papercut matches ID prefix '{prefix}'"),
            "Run `papercuts list --status all` and retry with a listed ID.",
        )),
        [id] => Ok(id.clone()),
        _ => Err(AppError::ambiguous_id(prefix, candidates)),
    }
}

pub fn canonical_ids(values: &[String]) -> AppResult<BTreeSet<String>> {
    values
        .iter()
        .map(|value| {
            let valid = value.len() == 15
                && value.starts_with("pc_")
                && value[3..].bytes().all(|byte| byte.is_ascii_hexdigit())
                && value[3..]
                    .bytes()
                    .all(|byte| !byte.is_ascii_alphabetic() || byte.is_ascii_lowercase());
            if !valid {
                return Err(AppError::invalid_argument(
                    format!("invalid canonical papercut ID '{value}'"),
                    "Pass a full canonical lowercase ID such as `pc_a1b2c3d4e5f6`.",
                ));
            }
            Ok(value.clone())
        })
        .collect()
}
