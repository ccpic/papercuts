use super::ids;
use crate::error::{AppError, AppResult};
use crate::store;
use crate::{
    BlockRecord, Blockage, ItemStatus, ListItem, UnblockRecord, format_timestamp, resolve_agent,
};
use jiff::Timestamp;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub enum Transition {
    Block,
    Unblock,
}

pub struct TransitionResult {
    pub changed_ids: Vec<String>,
    pub unchanged_ids: Vec<String>,
    pub records: Vec<ListItem>,
    pub file: PathBuf,
    pub agent_source: String,
}

#[derive(Serialize)]
#[serde(untagged)]
enum StatusEvent {
    Block(BlockRecord),
    Unblock(UnblockRecord),
}

pub struct TransitionArgs {
    pub ids: Vec<String>,
    pub agent: Option<String>,
    pub note: Option<String>,
    pub dry_run: bool,
}

pub fn apply(
    args: TransitionArgs,
    transition: Transition,
    file: Option<PathBuf>,
    now: Timestamp,
) -> AppResult<TransitionResult> {
    let prefixes: Vec<_> = args
        .ids
        .iter()
        .map(|id| ids::normalize_prefix(id))
        .collect::<AppResult<_>>()?;
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
                    .ok_or_else(|| {
                        AppError::internal("matched papercut disappeared during transition")
                    })
            })
            .collect::<AppResult<Vec<_>>>()?;
        let resolved_ids: Vec<_> = matched_ids
            .iter()
            .zip(&records)
            .filter(|(_, item)| item.status == ItemStatus::Resolved)
            .map(|(id, _)| id.clone())
            .collect();
        if !resolved_ids.is_empty() {
            let verb = match transition {
                Transition::Block => "block",
                Transition::Unblock => "unblock",
            };
            return Err(AppError::invalid_input(
                format!(
                    "cannot {verb} resolved papercuts: {}",
                    resolved_ids.join(", ")
                ),
                "Pass only open or blocked IDs; resolved state is terminal.",
            ));
        }

        let mut changed_ids = Vec::new();
        let mut unchanged_ids = Vec::new();
        let mut events = Vec::new();
        for (id, item) in matched_ids.iter().zip(records.iter_mut()) {
            let should_change = match transition {
                Transition::Block => item.status == ItemStatus::Open,
                Transition::Unblock => item.status == ItemStatus::Blocked,
            };
            if !should_change {
                unchanged_ids.push(id.clone());
                continue;
            }
            changed_ids.push(id.clone());
            match transition {
                Transition::Block => {
                    item.status = ItemStatus::Blocked;
                    item.blockage = Some(Blockage {
                        ts: ts.clone(),
                        agent: agent.clone(),
                        note: args.note.clone(),
                    });
                    events.push(StatusEvent::Block(BlockRecord {
                        kind: "block".into(),
                        id: id.clone(),
                        ts: ts.clone(),
                        agent: agent.clone(),
                        note: args.note.clone(),
                    }));
                }
                Transition::Unblock => {
                    item.status = ItemStatus::Open;
                    item.blockage = None;
                    events.push(StatusEvent::Unblock(UnblockRecord {
                        kind: "unblock".into(),
                        id: id.clone(),
                        ts: ts.clone(),
                        agent: agent.clone(),
                        note: args.note.clone(),
                    }));
                }
            }
        }
        if !args.dry_run && !events.is_empty() {
            store::append_json_batch(log, &resolved.path, &bytes, &events)?;
        }
        Ok((changed_ids, unchanged_ids, records))
    };
    let (changed_ids, unchanged_ids, records) = if args.dry_run {
        store::with_shared(&resolved.path, action)
    } else {
        store::with_exclusive(&resolved.path, false, action)
    }?;
    Ok(TransitionResult {
        changed_ids,
        unchanged_ids,
        records,
        file: resolved.path,
        agent_source: source.into(),
    })
}
