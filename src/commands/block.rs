use super::status_transition::{self, Transition, TransitionArgs};
use crate::ListItem;
use crate::cli::BlockArgs;
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockData {
    pub changed: bool,
    pub blocked_now_ids: Vec<String>,
    pub already_blocked_ids: Vec<String>,
    pub would_block_ids: Vec<String>,
    pub records: Vec<ListItem>,
}

pub fn run(args: BlockArgs, file: Option<PathBuf>, pretty: bool, now: Timestamp) -> AppResult<i32> {
    let dry_run = args.dry_run;
    let result = status_transition::apply(
        TransitionArgs {
            ids: args.ids,
            agent: args.agent,
            note: args.note,
            dry_run,
        },
        Transition::Block,
        file,
        now,
    )?;
    let blocked_now_ids = if dry_run {
        Vec::new()
    } else {
        result.changed_ids.clone()
    };
    let would_block_ids = if dry_run {
        result.changed_ids
    } else {
        Vec::new()
    };
    let changed = !blocked_now_ids.is_empty();
    let mut meta = Meta::new();
    meta.file = Some(result.file.to_string_lossy().into_owned());
    meta.agent_source = Some(result.agent_source);
    if !result.unchanged_ids.is_empty() {
        meta.warnings.push(format!(
            "already blocked: {}",
            result.unchanged_ids.join(", ")
        ));
    }
    if dry_run {
        meta.warnings
            .push("dry run; no block event appended".into());
    }
    output::write_success(
        BlockData {
            changed,
            blocked_now_ids,
            already_blocked_ids: result.unchanged_ids,
            would_block_ids,
            records: result.records,
        },
        pretty,
        meta,
    )
    .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    Ok(0)
}
