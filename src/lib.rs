pub mod cli;
pub mod commands;
pub mod error;
pub mod output;
pub mod store;

use crate::error::{AppError, AppResult};
use jiff::{SignedDuration, Timestamp, Unit};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

pub const CONTRACT_VERSION: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Minor,
    Major,
    Blocker,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Minor => "minor",
            Self::Major => "major",
            Self::Blocker => "blocker",
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Self::Minor => 0,
            Self::Major => 1,
            Self::Blocker => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CutRecord {
    pub kind: String,
    pub id: String,
    pub ts: String,
    pub agent: String,
    pub text: String,
    pub tags: Vec<String>,
    pub severity: Severity,
    pub cwd: String,
    pub repo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Evidence>,
    /// Debt only: the party that owns the ledger this record tracks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Debt only: the declared ledger size at registration time. The crate stores
    /// it without comparing it; ratchet decisions belong to the repository's own checkers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_count: Option<u64>,
    /// Debt only: repository-relative pointer to the authoritative ledger, and the
    /// identity the debt ID is derived from (one ledger, one record).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum ResolutionReason {
    #[default]
    #[value(skip)]
    Legacy,
    Fixed,
    AlreadyFixed,
    Duplicate,
    FalseObservation,
    Obsolete,
    /// 有意不修:记录真实存在、需求仍有效,但评估后决定不修复。
    #[serde(rename = "won't-fix")]
    #[value(name = "won't-fix")]
    WontFix,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveRecord {
    pub kind: String,
    pub id: String,
    pub ts: String,
    pub agent: String,
    #[serde(default)]
    pub reason: ResolutionReason,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockRecord {
    pub kind: String,
    pub id: String,
    pub ts: String,
    pub agent: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnblockRecord {
    pub kind: String,
    pub id: String,
    pub ts: String,
    pub agent: String,
    pub note: Option<String>,
}

/// Reopens a resolved debt with a fresh declared baseline. The only channel that
/// can leave a terminal state, because debt recurs (a ratchet baseline can grow back).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebaselineRecord {
    pub kind: String,
    pub id: String,
    pub ts: String,
    pub agent: String,
    pub baseline_count: u64,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    pub ts: String,
    pub agent: String,
    pub reason: ResolutionReason,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blockage {
    pub ts: String,
    pub agent: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListItem {
    #[serde(flatten)]
    pub cut: CutRecord,
    pub status: ItemStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<Resolution>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blockage: Option<Blockage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemStatus {
    Open,
    Blocked,
    Resolved,
}

pub fn effective_now() -> AppResult<Timestamp> {
    let timestamp = match std::env::var("PAPERCUTS_NOW") {
        Ok(value) if !value.is_empty() => value.parse::<Timestamp>().map_err(|_| {
            AppError::config(
                "PAPERCUTS_NOW must be a full RFC3339 timestamp",
                "Set PAPERCUTS_NOW to a value like 2026-07-09T18:30:00Z or unset it.",
            )
        })?,
        Ok(_) | Err(std::env::VarError::NotPresent) => Timestamp::now(),
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(AppError::config(
                "PAPERCUTS_NOW is not valid UTF-8",
                "Set PAPERCUTS_NOW to a full RFC3339 timestamp or unset it.",
            ));
        }
    };
    timestamp
        .round(Unit::Millisecond)
        .map_err(|error| AppError::internal(error.to_string()))
}

pub fn format_timestamp(timestamp: Timestamp) -> String {
    format!("{timestamp:.3}")
}

pub fn parse_since(value: &str, now: Timestamp) -> AppResult<Timestamp> {
    if let Some((number, unit)) = value.split_at_checked(value.len().saturating_sub(1))
        && !number.is_empty()
        && number.bytes().all(|byte| byte.is_ascii_digit())
        && matches!(unit, "d" | "h")
    {
        let amount = number.parse::<i64>().map_err(|_| {
            AppError::invalid_argument(
                format!("invalid --since value '{value}'"),
                "Use a full RFC3339 timestamp, Nd, or Nh.",
            )
        })?;
        let hours = if unit == "d" {
            amount.checked_mul(24)
        } else {
            Some(amount)
        }
        .ok_or_else(|| {
            AppError::invalid_argument(
                format!("--since value '{value}' is too large"),
                "Use a smaller Nd or Nh duration.",
            )
        })?;
        return now
            .checked_sub(SignedDuration::from_hours(hours))
            .map_err(|_| {
                AppError::invalid_argument(
                    format!("--since value '{value}' is outside the supported range"),
                    "Use a smaller Nd or Nh duration.",
                )
            });
    }

    value.parse::<Timestamp>().map_err(|_| {
        AppError::invalid_argument(
            format!("invalid --since value '{value}'"),
            "Use a full RFC3339 timestamp such as 2026-07-09T18:30:00Z, or a relative value such as 7d or 12h.",
        )
    })
}

pub const KIND_CUT: &str = "cut";
pub const KIND_DEBT: &str = "debt";

pub fn compute_id(
    ts: &str,
    agent: &str,
    text: &str,
    severity: Severity,
    tags: &[String],
) -> String {
    let mut tags = tags.to_vec();
    tags.sort();
    let joined_tags = tags.join(",");
    let mut hash = Sha256::new();
    for field in [ts, agent, text, severity.as_str(), joined_tags.as_str()] {
        hash.update((field.len() as u32).to_le_bytes());
        hash.update(field.as_bytes());
    }
    let digest = hash.finalize();
    let mut id = String::with_capacity(15);
    id.push_str("pc_");
    for byte in &digest[..6] {
        write!(&mut id, "{byte:02x}").expect("writing to a String cannot fail");
    }
    id
}

/// Debt IDs derive from `path` alone so that one ledger yields exactly one stable
/// record: re-running a migration at a new wall-clock time must not stack duplicates.
pub fn compute_debt_id(path: &str) -> String {
    let mut hash = Sha256::new();
    hash.update((path.len() as u32).to_le_bytes());
    hash.update(path.as_bytes());
    let digest = hash.finalize();
    let mut id = String::with_capacity(15);
    id.push_str("pc_");
    for byte in &digest[..6] {
        write!(&mut id, "{byte:02x}").expect("writing to a String cannot fail");
    }
    id
}

pub fn resolve_agent(flag: Option<String>) -> (String, &'static str) {
    if let Some(agent) = flag.filter(|value| !value.is_empty()) {
        return (agent, "flag");
    }
    if let Ok(agent) = std::env::var("PAPERCUTS_AGENT")
        && !agent.is_empty()
    {
        return (agent, "env");
    }
    if std::env::var_os("CLAUDECODE").is_some() {
        return ("claude-code".into(), "detected");
    }
    if std::env::vars_os().any(|(key, _)| key.to_string_lossy().starts_with("CODEX_")) {
        return ("codex".into(), "detected");
    }
    if std::env::vars_os().any(|(key, _)| key.to_string_lossy().starts_with("CURSOR_")) {
        return ("cursor".into(), "detected");
    }
    ("unknown".into(), "default")
}
