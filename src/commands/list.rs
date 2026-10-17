use super::ids;
use crate::cli::{KindFilter, ListArgs, ListProjection, OutputFormat, StatusFilter};
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use crate::store;
use crate::{Blockage, ItemStatus, ListItem, Severity, parse_since};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct ListData {
    pub items: Vec<ListItem>,
    pub count: usize,
    pub total: usize,
    pub truncated: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TriageListData {
    pub items: Vec<TriageItem>,
    pub count: usize,
    pub total: usize,
    pub truncated: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TriageItem {
    pub id: String,
    pub ts: String,
    pub text: String,
    pub tags: Vec<String>,
    pub severity: Severity,
    pub cwd: String,
    pub repo: Option<String>,
    pub status: ItemStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blockage: Option<Blockage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl From<ListItem> for TriageItem {
    fn from(item: ListItem) -> Self {
        Self {
            id: item.cut.id,
            ts: item.cut.ts,
            text: item.cut.text,
            tags: item.cut.tags,
            severity: item.cut.severity,
            cwd: item.cut.cwd,
            repo: item.cut.repo,
            status: item.status,
            blockage: item.blockage,
            owner: item.cut.owner,
            baseline_count: item.cut.baseline_count,
            path: item.cut.path,
        }
    }
}

pub fn run(args: ListArgs, file: Option<PathBuf>, pretty: bool, now: Timestamp) -> AppResult<i32> {
    if args.format == OutputFormat::Md && args.projection == ListProjection::Triage {
        return Err(AppError::invalid_argument(
            "--projection triage is only available with --format json",
            "Use `--format json --projection triage` or omit --projection for Markdown.",
        ));
    }
    let ids = ids::canonical_ids(&args.ids)?;
    let resolved = store::discover(file)?;
    let mut warnings = Vec::new();
    let folded = match store::with_shared(&resolved.path, |log| {
        let bytes = store::read_bytes(log, &resolved.path)?;
        Ok(store::fold_bytes(&bytes))
    }) {
        Ok(folded) => folded,
        Err(error) if error.code == "not_found" && error.exit_code == 66 => {
            if resolved.explicit {
                return Err(AppError::not_found(
                    format!("papercuts file not found: {}", resolved.path.display()),
                    "Pass an existing --file PATH or run `papercuts add` to create a discovered default file.",
                ));
            }
            warnings.push("no papercuts file yet; papercuts add creates it".into());
            store::FoldResult::default()
        }
        Err(error) => return Err(error),
    };
    warnings.extend(folded.warnings);
    let since = args
        .since
        .as_deref()
        .map(|value| parse_since(value, now))
        .transpose()?;
    let mut items: Vec<_> = folded
        .items
        .into_iter()
        .filter(|item| match args.status {
            StatusFilter::Open => item.status == ItemStatus::Open,
            StatusFilter::Blocked => item.status == ItemStatus::Blocked,
            StatusFilter::Resolved => item.status == ItemStatus::Resolved,
            StatusFilter::All => true,
        })
        .filter(|item| match args.kind {
            KindFilter::Cut => item.cut.kind == crate::KIND_CUT,
            KindFilter::Debt => item.cut.kind == crate::KIND_DEBT,
            KindFilter::All => {
                item.cut.kind == crate::KIND_CUT || item.cut.kind == crate::KIND_DEBT
            }
        })
        .filter(|item| {
            args.owner
                .as_ref()
                .is_none_or(|owner| item.cut.owner.as_ref() == Some(owner))
        })
        .filter(|item| ids.is_empty() || ids.contains(&item.cut.id))
        .filter(|item| {
            args.agent
                .as_ref()
                .is_none_or(|agent| &item.cut.agent == agent)
        })
        .filter(|item| {
            args.tag
                .as_ref()
                .is_none_or(|tag| item.cut.tags.contains(tag))
        })
        .filter(|item| {
            args.severity
                .is_none_or(|severity| item.cut.severity == severity)
        })
        .filter(|item| {
            since.is_none_or(|threshold| {
                item.cut
                    .ts
                    .parse::<Timestamp>()
                    .is_ok_and(|timestamp| timestamp >= threshold)
            })
        })
        .collect();
    let total = items.len();
    items.truncate(args.limit);
    let count = items.len();
    let truncated = total > count;
    if items.is_empty() {
        warnings.push("no papercuts matched; try --status all or broader filters".into());
    }
    if args.format == OutputFormat::Md {
        write_markdown(&items, &warnings)?;
    } else {
        let mut meta = Meta::new();
        meta.file = Some(resolved.path.to_string_lossy().into_owned());
        meta.warnings = warnings;
        match args.projection {
            ListProjection::Full => output::write_success(
                ListData {
                    items,
                    count,
                    total,
                    truncated,
                },
                pretty,
                meta,
            ),
            ListProjection::Triage => output::write_success(
                TriageListData {
                    items: items.into_iter().map(TriageItem::from).collect(),
                    count,
                    total,
                    truncated,
                },
                pretty,
                meta,
            ),
        }
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    }
    Ok(0)
}

fn write_markdown(items: &[ListItem], warnings: &[String]) -> AppResult<()> {
    let mut output = std::io::BufWriter::new(std::io::stdout().lock());
    for severity in [Severity::Blocker, Severity::Major, Severity::Minor] {
        let matching: Vec<_> = items
            .iter()
            .filter(|item| item.cut.severity == severity)
            .collect();
        if matching.is_empty() {
            continue;
        }
        writeln!(
            output,
            "## {}",
            match severity {
                Severity::Blocker => "Blocker",
                Severity::Major => "Major",
                Severity::Minor => "Minor",
            }
        )
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
        for item in matching {
            let id = if item.status == ItemStatus::Resolved {
                format!("~~{}~~", item.cut.id)
            } else {
                item.cut.id.clone()
            };
            let tags = if item.cut.tags.is_empty() {
                String::new()
            } else {
                format!(" ({})", item.cut.tags.join(","))
            };
            writeln!(
                output,
                "- [{id}] {} — {}, {}{tags}",
                item.cut.text, item.cut.agent, item.cut.ts
            )
            .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
        }
    }
    for warning in warnings {
        writeln!(output, "> note: {warning}")
            .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    }
    Ok(())
}
