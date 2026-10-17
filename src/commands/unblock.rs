use super::status_transition::{self, Transition, TransitionArgs};
use crate::ListItem;
use crate::cli::UnblockArgs;
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct UnblockData {
    pub changed: bool,
    pub unblocked_now_ids: Vec<String>,
    pub already_open_ids: Vec<String>,
    pub would_unblock_ids: Vec<String>,
    pub records: Vec<ListItem>,
}

pub fn run(
    args: UnblockArgs,
    file: Option<PathBuf>,
    pretty: bool,
    now: Timestamp,
) -> AppResult<i32> {
    let dry_run = args.dry_run;
    let result = status_transition::apply(
        TransitionArgs {
            ids: args.ids,
            agent: args.agent,
            note: args.note,
            dry_run,
        },
        Transition::Unblock,
        file,
        now,
    )?;
    let unblocked_now_ids = if dry_run {
        Vec::new()
    } else {
        result.changed_ids.clone()
    };
    let would_unblock_ids = if dry_run {
        result.changed_ids
    } else {
        Vec::new()
    };
    let changed = !unblocked_now_ids.is_empty();
    let mut meta = Meta::new();
    meta.file = Some(result.file.to_string_lossy().into_owned());
    meta.agent_source = Some(result.agent_source);
    if !result.unchanged_ids.is_empty() {
        meta.warnings
            .push(format!("already open: {}", result.unchanged_ids.join(", ")));
    }
    if dry_run {
        meta.warnings
            .push("dry run; no unblock event appended".into());
    }
    output::write_success(
        UnblockData {
            changed,
            unblocked_now_ids,
            already_open_ids: result.unchanged_ids,
            would_unblock_ids,
            records: result.records,
        },
        pretty,
        meta,
    )
    .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    Ok(0)
}
