use crate::CONTRACT_VERSION;
use crate::cli::SchemaTarget;
use crate::error;
use serde_json::{Value, json};

pub fn contract(target: SchemaTarget) -> Value {
    let records = json!({
        "cut": {"kind":"cut","id":"pc_<12 lowercase hex>","ts":"RFC3339 UTC milliseconds","agent":"string","text":"string <= 10000 bytes","tags":["string"],"severity":"minor|major|blocker","cwd":"absolute path","repo":"absolute path|null","evidence":"optional {cmd:string,exit:integer,stderr:string,note:string}; absent fields omitted; values best-effort redacted"},
        "resolve": {"kind":"resolve","id":"pc_<12 lowercase hex>","ts":"RFC3339 UTC milliseconds","agent":"string","reason":"fixed|already-fixed|duplicate|false-observation|obsolete|won't-fix|legacy","note":"string|null"},
        "block": {"kind":"block","id":"pc_<12 lowercase hex>","ts":"RFC3339 UTC milliseconds","agent":"string","note":"string|null"},
        "unblock": {"kind":"unblock","id":"pc_<12 lowercase hex>","ts":"RFC3339 UTC milliseconds","agent":"string","note":"string|null"},
        "list_item": {"cut":"all cut fields","status":"open|blocked|resolved","resolution":"{ts,agent,reason,note}|omitted","blockage":"{ts,agent,note}|omitted"}
    });
    let errors = json!({
        "shape": {"ok":false,"error":{"code":"string","message":"string","details":{},"retryable":false,"suggested_fix":"string"},"meta":{"contract":CONTRACT_VERSION}},
        "codes": error::error_codes()
    });
    let exit_codes: Value = json!(error::exit_code_map());
    match target {
        SchemaTarget::Record => json!({"contract":CONTRACT_VERSION,"records":records}),
        SchemaTarget::Error => json!({"contract":CONTRACT_VERSION,"errors":errors}),
        SchemaTarget::ExitCodes => json!({"contract":CONTRACT_VERSION,"exit_codes":exit_codes}),
        SchemaTarget::All => json!({
            "contract": CONTRACT_VERSION,
            "success_envelope": {"ok":true,"data":"command-specific object","meta":{"contract":CONTRACT_VERSION,"file":"absolute path where relevant","agent_source":"flag|env|detected|default where relevant","warnings":["string; omitted when empty"]}},
            "commands": {
                "add": {"alias":["log"],"positional":"TEXT or -; optional when stdin is piped","flags":{"--agent":"NAME","--tag":"TAG; repeatable","--severity":"minor|major|blocker; default minor","--cmd":"TEXT; optional evidence command; leading hyphens accepted","--exit":"N; optional evidence exit status","--stderr-file":"PATH; regular UTF-8 file up to 1 MiB, redacted then stored up to 4096 bytes; symlinks follow targets","--evidence":"TEXT; optional evidence note; leading hyphens accepted","--dry-run":"boolean"},"output":"{changed,record}","read_only":false,"appends":true,"destructive":false},
                "list": {"flags":{"--status":"open|blocked|resolved|all; default open","--id":"full canonical lowercase ID; repeatable","--agent":"NAME","--tag":"TAG","--severity":"minor|major|blocker","--since":"full RFC3339|Nd|Nh","--limit":"N; default 50","--format":"json|md; default json","--projection":"full|triage; default full; triage requires json"},"output":"full: {items,count,total,truncated}; triage items contain id,ts,text,tags,severity,cwd,repo,status,blockage; md is raw markdown","read_only":true,"appends":false,"destructive":false},
                "resolve": {"positional":"one or more IDs; optional pc_ plus at least 4 hex digits each","flags":{"--reason":"fixed|already-fixed|duplicate|false-observation|obsolete|won't-fix; default fixed","--note":"TEXT; leading hyphens accepted","--agent":"NAME","--dry-run":"boolean"},"output":{"one":"{changed,resolved_now_ids,already_resolved_ids,would_resolve_ids,record}","two_or_more":"{changed,resolved_now_ids,already_resolved_ids,would_resolve_ids,records}; IDs are canonicalized, sorted, and duplicate inputs collapse"},"read_only":false,"appends":true,"destructive":false},
                "block": {"positional":"one or more IDs; optional pc_ plus at least 4 hex digits each","flags":{"--note":"TEXT; leading hyphens accepted","--agent":"NAME","--dry-run":"boolean"},"output":"{changed,blocked_now_ids,already_blocked_ids,would_block_ids,records}","read_only":false,"appends":true,"destructive":false},
                "unblock": {"positional":"one or more IDs; optional pc_ plus at least 4 hex digits each","flags":{"--note":"TEXT; leading hyphens accepted","--agent":"NAME","--dry-run":"boolean"},"output":"{changed,unblocked_now_ids,already_open_ids,would_unblock_ids,records}","read_only":false,"appends":true,"destructive":false},
                "schema": {"positional":"all|record|error|exit-codes; default all","read_only":true,"appends":false,"destructive":false},
                "doctor": {"flags":{},"output":"{healthy,findings,checked_lines}","exit_codes":{"0":"healthy","1":"findings"},"read_only":true,"appends":false,"destructive":false}
            },
            "global_flags": ["--file <PATH>","--pretty"],
            "env": {
                "PAPERCUTS_FILE":"log-file override",
                "PAPERCUTS_AGENT":"agent-name fallback",
                "PAPERCUTS_NOW":"full RFC3339 clock override"
            },
            "records": records,
            "id": {"prefix":"pc_","hex_digits":12,"hash":"SHA-256 first 6 bytes","fields_in_order":["ts","agent","text","severity","sorted tags joined with comma"],"encoding":"u32 little-endian UTF-8 byte length before each field"},
            "discovery": ["--file","PAPERCUTS_FILE","nearest .git directory or file then <root>/.papercuts.jsonl","$HOME/.papercuts/log.jsonl"],
            "errors": errors,
            "exit_codes": exit_codes,
            "storage": {"format":"append-only JSONL","locking":"local filesystems only; 50 retries x 100ms","durability":"best effort; no fsync per append"}
        }),
    }
}
