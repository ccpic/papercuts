use super::ids;
use crate::cli::RebaselineArgs;
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use crate::store;
use crate::{ItemStatus, KIND_DEBT, ListItem, RebaselineRecord, format_timestamp, resolve_agent};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct RebaselineData {
    pub changed: bool,
    pub rebaselined_now_ids: Vec<String>,
    pub already_current_ids: Vec<String>,
    pub would_rebaseline_ids: Vec<String>,
    pub records: Vec<ListItem>,
}

pub fn run(
    args: RebaselineArgs,
    file: Option<PathBuf>,
    pretty: bool,
    now: Timestamp,
) -> AppResult<i32> {
    let prefixes: Vec<_> = args
        .ids
        .iter()
        .map(|id| ids::normalize_prefix(id))
        .collect::<AppResult<_>>()?;
    if args.note.trim().is_empty() {
        return Err(AppError::invalid_input(
            "rebaseline requires a non-empty --note",
            "Pass --note <TEXT> recording why the baseline moved.",
        ));
    }
    if args
        .agent
        .as_deref()
        .is_some_and(|agent| agent.trim().is_empty())
    {
        return Err(AppError::invalid_input(
            "agent name cannot be empty or whitespace-only",
            "Pass a non-empty --agent NAME or omit the flag.",
        ));
    }
    let resolved = store::discover(file)?;
    let (agent, source) = resolve_agent(args.agent);
    let ts = format_timestamp(now);
    let baseline_count = args.baseline_count;
    let note = args.note;
    let action = |log: &mut std::fs::File| -> AppResult<(Vec<String>, Vec<String>, Vec<ListItem>)> {
        let bytes = store::read_bytes(log, &resolved.path)?;
        let folded = store::fold_bytes(&bytes);
        let mut matched_ids = prefixes
            .iter()
            .map(|prefix| ids::match_id(prefix, &folded.items))
            .collect::<AppResult<Vec<_>>>()?;
        matched_ids.sort();
        matched_ids.dedup();
        let mut records = matched_ids
            .iter()
            .map(|id| {
                folded
                    .items
                    .iter()
                    .find(|item| item.cut.id == *id)
                    .cloned()
                    .ok_or_else(|| AppError::internal("matched debt disappeared during rebaseline"))
            })
            .collect::<AppResult<Vec<_>>>()?;
        // Rebaseline is debt-only: a cut has no ledger to re-measure, and its resolved
        // state must stay terminal.
        let non_debt: Vec<_> = matched_ids
            .iter()
            .zip(&records)
            .filter(|(_, item)| item.cut.kind != KIND_DEBT)
            .map(|(id, _)| id.clone())
            .collect();
        if !non_debt.is_empty() {
            return Err(AppError::invalid_input(
                format!(
                    "cannot rebaseline non-debt records: {}",
                    non_debt.join(", ")
                ),
                "Pass debt IDs only; rebaseline reopens a ledger baseline.",
            ));
        }
        let already_current_ids: Vec<_> = matched_ids
            .iter()
            .zip(&records)
            .filter(|(_, item)| {
                item.status != ItemStatus::Resolved
                    && item.cut.baseline_count == Some(baseline_count)
            })
            .map(|(id, _)| id.clone())
            .collect();
        let changed_ids: Vec<_> = matched_ids
            .iter()
            .zip(&records)
            .filter(|(id, _)| !already_current_ids.contains(id))
            .map(|(id, _)| id.clone())
            .collect();

        let mut events = Vec::new();
        for (id, item) in matched_ids.iter().zip(records.iter_mut()) {
            if already_current_ids.contains(id) {
                continue;
            }
            item.status = ItemStatus::Open;
            item.resolution = None;
            item.blockage = None;
            item.cut.baseline_count = Some(baseline_count);
            events.push(RebaselineRecord {
                kind: "rebaseline".into(),
                id: id.clone(),
                ts: ts.clone(),
                agent: agent.clone(),
                baseline_count,
                note: note.clone(),
            });
        }
        if !args.dry_run && !events.is_empty() {
            store::append_json_batch(log, &resolved.path, &bytes, &events)?;
        }
        Ok((changed_ids, already_current_ids, records))
    };
    let (changed_ids, already_current_ids, records) = if args.dry_run {
        store::with_shared(&resolved.path, action)
    } else {
        store::with_exclusive(&resolved.path, false, action)
    }?;
    let rebaselined_now_ids = if args.dry_run {
        Vec::new()
    } else {
        changed_ids.clone()
    };
    let would_rebaseline_ids = if args.dry_run {
        changed_ids
    } else {
        Vec::new()
    };
    let changed = !rebaselined_now_ids.is_empty();
    let mut meta = Meta::new();
    meta.file = Some(resolved.path.to_string_lossy().into_owned());
    meta.agent_source = Some(source.into());
    if !already_current_ids.is_empty() {
        meta.warnings.push(format!(
            "already at the declared baseline: {}",
            already_current_ids.join(", ")
        ));
    }
    if args.dry_run {
        meta.warnings
            .push("dry run; no rebaseline event appended".into());
    }
    output::write_success(
        RebaselineData {
            changed,
            rebaselined_now_ids,
            already_current_ids,
            would_rebaseline_ids,
            records,
        },
        pretty,
        meta,
    )
    .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    Ok(0)
}
