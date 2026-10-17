use super::ids;
use crate::cli::ResolveArgs;
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use crate::store;
use crate::{ItemStatus, ListItem, Resolution, ResolveRecord, format_timestamp, resolve_agent};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolveData {
    pub changed: bool,
    pub resolved_now_ids: Vec<String>,
    pub already_resolved_ids: Vec<String>,
    pub would_resolve_ids: Vec<String>,
    pub record: ListItem,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolveManyData {
    pub changed: bool,
    pub resolved_now_ids: Vec<String>,
    pub already_resolved_ids: Vec<String>,
    pub would_resolve_ids: Vec<String>,
    pub records: Vec<ListItem>,
}

pub fn run(
    args: ResolveArgs,
    file: Option<PathBuf>,
    pretty: bool,
    now: Timestamp,
) -> AppResult<i32> {
    let prefixes: Vec<_> = args
        .ids
        .iter()
        .map(|id| ids::normalize_prefix(id))
        .collect::<AppResult<_>>()?;
    let resolved = store::discover(file)?;
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
    let (agent, source) = resolve_agent(args.agent);
    let ts = format_timestamp(now);
    let reason = args.reason;
    let note = args.note;
    let many = args.ids.len() > 1;
    let action = |log: &mut std::fs::File| -> AppResult<(Vec<String>, Vec<String>, Vec<ListItem>)> {
        let bytes = store::read_bytes(log, &resolved.path)?;
        let folded = store::fold_bytes(&bytes);
        let mut matched_ids = prefixes
            .iter()
            .map(|prefix| ids::match_id(prefix, &folded.items))
            .collect::<AppResult<Vec<_>>>()?;
        matched_ids.sort();
        matched_ids.dedup();
        let mut items = matched_ids
            .iter()
            .map(|id| {
                folded
                    .items
                    .iter()
                    .find(|item| item.cut.id == *id)
                    .cloned()
                    .ok_or_else(|| {
                        AppError::internal("matched papercut disappeared during resolution")
                    })
            })
            .collect::<AppResult<Vec<_>>>()?;
        let already_resolved_ids: Vec<_> = matched_ids
            .iter()
            .zip(&items)
            .filter(|(_, item)| item.status == ItemStatus::Resolved)
            .map(|(id, _)| id.clone())
            .collect();
        let open_ids: Vec<_> = matched_ids
            .iter()
            .zip(&items)
            .filter(|(_, item)| item.status != ItemStatus::Resolved)
            .map(|(id, _)| id.clone())
            .collect();

        let mut events = Vec::new();
        for (id, item) in matched_ids.iter().zip(items.iter_mut()) {
            if item.status == ItemStatus::Resolved {
                continue;
            }
            item.status = ItemStatus::Resolved;
            item.blockage = None;
            item.resolution = Some(Resolution {
                ts: ts.clone(),
                agent: agent.clone(),
                reason,
                note: note.clone(),
            });
            events.push(ResolveRecord {
                kind: "resolve".into(),
                id: id.clone(),
                ts: ts.clone(),
                agent: agent.clone(),
                reason,
                note: note.clone(),
            });
        }
        if !args.dry_run && !events.is_empty() {
            store::append_json_batch(log, &resolved.path, &bytes, &events)?;
        }
        Ok((open_ids, already_resolved_ids, items))
    };
    let (open_ids, already_resolved_ids, records) = if args.dry_run {
        store::with_shared(&resolved.path, action)
    } else {
        store::with_exclusive(&resolved.path, false, action)
    }?;
    let resolved_now_ids = if args.dry_run {
        Vec::new()
    } else {
        open_ids.clone()
    };
    let would_resolve_ids = if args.dry_run { open_ids } else { Vec::new() };
    let changed = !resolved_now_ids.is_empty();
    let mut meta = Meta::new();
    meta.file = Some(resolved.path.to_string_lossy().into_owned());
    meta.agent_source = Some(source.into());
    if already_resolved_ids.len() == records.len() {
        meta.warnings.push("already resolved".into());
    } else if !already_resolved_ids.is_empty() {
        let noun = if already_resolved_ids.len() == 1 {
            "ID"
        } else {
            "IDs"
        };
        meta.warnings.push(format!(
            "already resolved: {} {noun} ({})",
            already_resolved_ids.len(),
            already_resolved_ids.join(", ")
        ));
    }
    if args.dry_run {
        meta.warnings
            .push("dry run; no resolve event appended".into());
    }
    if !many {
        output::write_success(
            ResolveData {
                changed,
                resolved_now_ids,
                already_resolved_ids,
                would_resolve_ids,
                record: records.into_iter().next().expect("one record"),
            },
            pretty,
            meta,
        )
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    } else {
        output::write_success(
            ResolveManyData {
                changed,
                resolved_now_ids,
                already_resolved_ids,
                would_resolve_ids,
                records,
            },
            pretty,
            meta,
        )
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    }
    Ok(0)
}
