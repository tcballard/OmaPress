//! Single-owner durable queue. Source changes block execution rather than publishing unreviewed edits.
use crate::{distribution, storage::*};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, path::Path, time::Duration};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub id: String,
    pub article: String,
    pub source_hash: String,
    pub title: String,
    pub at: DateTime<Utc>,
    pub timezone: String,
    pub targets: Vec<String>,
    pub status: String,
    pub results: Value,
    pub message: String,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Queue {
    schema: u32,
    jobs: Vec<Job>,
}
fn load(root: &Path) -> Result<Queue> {
    let p = safe_path(root, ".omapress/queue.json")?;
    if !p.exists() {
        return Ok(Queue {
            schema: 1,
            jobs: vec![],
        });
    }
    let q: Queue = serde_json::from_slice(&read_bounded(&p, 8 * 1024 * 1024)?)
        .context("Queue is corrupt; restore it before continuing")?;
    ensure!(q.schema == 1, "Unsupported queue schema");
    Ok(q)
}
fn save(root: &Path, q: &Queue) -> Result<()> {
    atomic_write(
        &safe_path(root, ".omapress/queue.json")?,
        &serde_json::to_vec_pretty(q)?,
    )
}
// Queue mutations use a separate lock from the adapter's publication lock.
fn queue_lock(root: &Path) -> Result<Lock> {
    lock(&safe_path(root, ".omapress/scheduler")?)
}
pub fn list(root: &Path) -> Result<Value> {
    let q = load(root)?;
    let heartbeat = safe_path(root, ".omapress/worker.json")?;
    let worker: Value = if heartbeat.exists() {
        serde_json::from_slice(&read_bounded(&heartbeat, 4096)?)?
    } else {
        Value::Null
    };
    Ok(
        json!({"jobs":q.jobs,"source_hash":snapshot(root)?.hash,"scope":"Website jobs deploy all Ready/Published articles. Source edits block queued jobs until cancelled and reviewed again.","worker_required":true,"worker":worker}),
    )
}
pub fn enqueue(root: &Path, args: Value) -> Result<Value> {
    let _q = queue_lock(root)?;
    let _p = lock(root)?;
    let article = args["article"].as_str().context("Missing article")?;
    let expected = args["expected_source_hash"]
        .as_str()
        .context("Missing source hash")?;
    let timezone = args["timezone"].as_str().unwrap_or("UTC");
    let at = parse_time(&args)?;
    let targets: Vec<String> = serde_json::from_value(args["targets"].clone())?;
    ensure!(
        !targets.is_empty() && targets.len() <= 2,
        "Select Website and/or X"
    );
    ensure!(
        targets.iter().all(|t| t == "website" || t == "x"),
        "Substack requires browser scheduling; unattended publishing is unavailable"
    );
    ensure!(
        targets
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == targets.len(),
        "Duplicate destination"
    );
    let p = distribution::plan(root, article, expected)?;
    ensure!(
        p["ready"] == true,
        "Mark the article Ready and resolve checks before scheduling"
    );
    if targets.iter().any(|t| t == "x") {
        ensure!(
            p["x_api_supported"] == true,
            "Unsupported X article formatting"
        );
    }
    let mut q = load(root)?;
    ensure!(
        q.jobs.len() < 1000,
        "Queue history limit reached; archive completed jobs first"
    );
    let id = args["id"].as_str().context("Missing idempotency ID")?;
    uuid::Uuid::parse_str(id).context("Invalid job ID")?;
    if let Some(old) = q.jobs.iter().find(|j| j.id == id) {
        ensure!(
            old.article == article
                && old.source_hash == expected
                && old.at == at
                && old.targets == targets
                && old.timezone == timezone,
            "Job ID was already used for a different request"
        );
        return list(root);
    }
    ensure!(
        !q.jobs.iter().any(|j| j.status != "cancelled"
            && j.targets.iter().any(|t| targets.contains(t)
                && (if t == "website" {
                    j.source_hash == expected
                        || ["queued", "running", "needs_review"].contains(&j.status.as_str())
                } else {
                    j.article == article
                }))),
        "This destination already has a queued or attempted job; inspect its result before scheduling again"
    );
    ensure!(at > Utc::now(), "Choose a future publishing time");
    q.jobs.push(Job {
        id: id.into(),
        article: article.into(),
        source_hash: expected.into(),
        title: p["title"].as_str().unwrap_or(article).into(),
        at,
        timezone: timezone.into(),
        targets,
        status: "queued".into(),
        results: json!({}),
        message: String::new(),
    });
    q.jobs.sort_by_key(|j| j.at);
    save(root, &q)?;
    list(root)
}
pub fn cancel(root: &Path, id: &str) -> Result<Value> {
    let _q = queue_lock(root)?;
    let mut q = load(root)?;
    let j = q
        .jobs
        .iter_mut()
        .find(|j| j.id == id)
        .context("Job not found")?;
    ensure!(
        j.status == "queued" || j.status == "blocked",
        "Only unattempted jobs can be cancelled; attempted destinations require reconciliation"
    );
    j.status = "cancelled".into();
    save(root, &q)?;
    list(root)
}
pub fn run_due(root: &Path) -> Result<Value> {
    let _q = queue_lock(root)?;
    atomic_write(
        &safe_path(root, ".omapress/worker.json")?,
        &serde_json::to_vec(&json!({"last_started_at":Utc::now().to_rfc3339()}))?,
    )?;
    let mut q = load(root)?;
    for i in 0..q.jobs.len() {
        if q.jobs[i].status == "running" {
            q.jobs[i].status = "needs_review".into();
            q.jobs[i].message =
                "Worker interrupted. Inspect destination receipts; no automatic retry.".into();
            save(root, &q)?;
        }
        if q.jobs[i].status != "queued" || q.jobs[i].at > Utc::now() {
            continue;
        }
        if Utc::now().signed_duration_since(q.jobs[i].at).num_minutes() > 60 {
            q.jobs[i].status = "blocked".into();
            q.jobs[i].message = "Missed by more than one hour; cancel and reschedule.".into();
            save(root, &q)?;
            continue;
        }
        let job = q.jobs[i].clone();
        let checked = snapshot(root).and_then(|s| s.expected(&job.source_hash));
        if let Err(e) = checked {
            q.jobs[i].status = "blocked".into();
            q.jobs[i].message = e.to_string();
            save(root, &q)?;
            continue;
        }
        q.jobs[i].status = "running".into();
        save(root, &q)?;
        let mut results = serde_json::Map::new();
        for target in &job.targets {
            let result = if target == "website" {
                crate::deploy::plan(root, &job.source_hash).and_then(|p| {
                    crate::deploy::publish(
                        root,
                        &job.source_hash,
                        p["expected_remote_head"]
                            .as_str()
                            .context("Missing website head")?,
                    )
                })
            } else {
                distribution::x_action(root, &job.article, &job.source_hash, true)
            };
            let value = match result {
                Ok(v) => {
                    json!({"ok":true,"result":if target == "x" { json!({"targets":v["targets"]}) } else { v }})
                }
                Err(e) => json!({"ok":false,"error":format!("{e:#}")}),
            };
            results.insert(target.clone(), value);
            q.jobs[i].results = json!(results);
            save(root, &q)?;
        }
        // An adapter can return a successful request with a pending remote outcome.
        let confirmed = results.iter().all(|(t, v)| {
            if t == "website" {
                v["ok"] == true && v["result"]["status"] == "published"
            } else {
                v["ok"] == true
                    && v["result"]["targets"].as_array().is_some_and(|ts| {
                        ts.iter()
                            .any(|x| x["target"] == "x" && x["status"] == "published")
                    })
            }
        });
        q.jobs[i].status = if confirmed {
            "completed"
        } else {
            "needs_review"
        }
        .into();
        save(root, &q)?;
        break; // Bound each timer invocation to one job; later due jobs run next tick.
    }
    list(root)
}
/// Authenticated SSH transport. Only a fixed CLI command reaches the remote shell;
/// paths, job arguments and content remain JSON on stdin. SSH aliases use known_hosts.
pub fn remote(_root: &Path, args: &Value) -> Result<Value> {
    let host = args["host"].as_str().context("Missing SSH host alias")?;
    ensure!(
        !host.is_empty()
            && host.len() <= 128
            && !host.starts_with('-')
            && host
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-@".contains(&c)),
        "Use an SSH host alias or user@host"
    );
    let path = args["path"]
        .as_str()
        .context("Missing remote publication path")?;
    ensure!(
        Path::new(path).is_absolute(),
        "Remote publication path must be absolute"
    );
    let command = args["command"]
        .as_str()
        .context("Missing remote operation")?;
    ensure!(
        [
            "queue-list",
            "queue-add",
            "queue-cancel",
            "inspect",
            "queue-upload",
            "queue-reconcile"
        ]
        .contains(&command),
        "Unsupported remote queue operation"
    );
    let payload = if command == "queue-upload" {
        let _lock = lock(_root)?;
        let s = snapshot(_root)?;
        s.expected(
            args["args"]["expected_source_hash"]
                .as_str()
                .context("Missing reviewed source hash")?,
        )?;
        let ledger = state(_root)?;
        ensure!(
            ledger.pending.is_none(),
            "Reconcile pending website deployment first"
        );
        let distribution_path = safe_path(_root, ".omapress/distribution.json")?;
        let history: Value = if distribution_path.exists() {
            serde_json::from_slice(&read_bounded(&distribution_path, 8 * 1024 * 1024)?)?
        } else {
            Value::Null
        };
        json!({"source_hash":s.hash,"files":s.files,"state":ledger,"distribution":history,"expected_remote_source_hash":args["args"]["expected_remote_source_hash"]})
    } else {
        args["args"].clone()
    };
    let request = json!({"schema":1,"command":if command == "queue-upload" { "queue-receive" } else { command },"path":path,"args":payload});
    ensure!(
        serde_json::to_vec(&request)?.len() <= 6 * 1024 * 1024,
        "Worker transfer exceeds 6 MiB encoded limit; reduce artwork size"
    );
    let text = crate::process::run_input(
        "ssh",
        &[
            "-o",
            "BatchMode=yes",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "ClearAllForwardings=yes",
            host,
            "omapress rpc",
        ],
        None,
        Duration::from_secs(45),
        &serde_json::to_vec(&request)?,
    )?;
    let response: Value = serde_json::from_str(&text).context("Invalid worker response")?;
    ensure!(
        response["schema"] == 1 && response["ok"] == true,
        "Remote worker: {}",
        response["error"]
            .as_str()
            .unwrap_or("incompatible response")
    );
    Ok(response["result"].clone())
}

/// Copy an initial reviewed publication to a new, dedicated worker directory.
/// Existing copies are immutable while scheduling; never overwrite remote history.
pub fn receive(root: &Path, args: &Value) -> Result<Value> {
    use std::{collections::BTreeMap, os::unix::fs::PermissionsExt};
    let expected = args["source_hash"]
        .as_str()
        .context("Missing transfer hash")?;
    ensure!(!root.is_symlink(), "Worker publication cannot be a symlink");
    let existing = root.exists();
    let _queue = if existing {
        Some(queue_lock(root)?)
    } else {
        None
    };
    let _publication = if existing { Some(lock(root)?) } else { None };
    if existing {
        let current = snapshot(root)?;
        if current.hash == expected {
            return list(root);
        }
        current.expected(
            args["expected_remote_source_hash"]
                .as_str()
                .context("Refresh the worker queue before replacing its source")?,
        )?;
        ensure!(
            load(root)?
                .jobs
                .iter()
                .all(|j| ["completed", "cancelled", "blocked"].contains(&j.status.as_str())),
            "Cancel pending jobs and reconcile attempted jobs before updating worker source"
        );
    }
    let parent = root.parent().context("Missing worker directory parent")?;
    ensure!(
        parent.is_dir(),
        "Create the private worker parent directory first"
    );
    let files: BTreeMap<String, Vec<u8>> = serde_json::from_value(args["files"].clone())?;
    ensure!(
        files.len() <= 30000 && tree_hash(&files) == expected,
        "Invalid publication transfer"
    );
    let stage = tempfile::Builder::new()
        .prefix(".omapress-upload-")
        .tempdir_in(parent)?;
    fs::set_permissions(stage.path(), fs::Permissions::from_mode(0o700))?;
    for (name, data) in &files {
        ensure!(
            name == "publication.toml"
                || ["content/", "media/", "themes/", "static/"]
                    .iter()
                    .any(|p| name.starts_with(p)),
            "Unexpected transfer path"
        );
        atomic_write(&safe_path(stage.path(), name)?, data)?;
    }
    snapshot(stage.path())?.expected(expected)?;
    // Existing worker history is authoritative after the first upload.
    if existing {
        fn copy_state(from: &Path, to: &Path) -> Result<()> {
            for entry in fs::read_dir(from)? {
                let entry = entry?;
                let ty = entry.file_type()?;
                ensure!(!ty.is_symlink(), "Symlink in worker state");
                let dest = to.join(entry.file_name());
                if ty.is_dir() {
                    fs::create_dir_all(&dest)?;
                    copy_state(&entry.path(), &dest)?;
                } else {
                    ensure!(ty.is_file(), "Special file in worker state");
                    atomic_write(&dest, &read_bounded(&entry.path(), 32 * 1024 * 1024)?)?;
                }
            }
            Ok(())
        }
        let dest = stage.path().join(".omapress");
        fs::create_dir_all(&dest)?;
        copy_state(&root.join(".omapress"), &dest)?;
    }
    for (key, name) in [
        ("state", "state.json"),
        ("distribution", "distribution.json"),
    ] {
        if !existing && !args[key].is_null() {
            atomic_write(
                &safe_path(stage.path(), &format!(".omapress/{name}"))?,
                &serde_json::to_vec(&args[key])?,
            )?;
        }
    }
    ensure!(
        state(stage.path())?.pending.is_none(),
        "Reconcile pending website deployment before transferring"
    );
    // Parent-level lock prevents two first uploads from racing.
    let _parent = lock(parent)?;
    if existing {
        use std::os::unix::ffi::OsStrExt;
        let from = std::ffi::CString::new(stage.path().as_os_str().as_bytes())?;
        let to = std::ffi::CString::new(root.as_os_str().as_bytes())?;
        // Linux atomic exchange preserves either complete source tree across interruption.
        let rc = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_EXCHANGE,
            )
        };
        ensure!(
            rc == 0,
            "Atomic worker update failed: {}",
            std::io::Error::last_os_error()
        );
    } else {
        ensure!(
            !root.exists(),
            "Worker directory appeared during upload; inspect it before retrying"
        );
        fs::rename(stage.path(), root)?;
    }
    fs::File::open(parent)?.sync_all()?;
    list(root)
}

/// Reconcile a stopped job against durable adapter receipts, never by resending.
pub fn reconcile(root: &Path, id: &str) -> Result<Value> {
    let _q = queue_lock(root)?;
    let _p = lock(root)?;
    let mut q = load(root)?;
    let job = q
        .jobs
        .iter_mut()
        .find(|j| j.id == id)
        .context("Job not found")?;
    ensure!(
        job.status == "needs_review",
        "This job does not require reconciliation"
    );
    let p = distribution::plan(root, &job.article, &job.source_hash)?;
    let state = state(root)?;
    let confirmed = job.targets.iter().all(|target| {
        if target == "website" {
            state.pending.is_none()
                && state
                    .deployments
                    .last()
                    .is_some_and(|d| d.source_hash == job.source_hash && d.verified_at.is_some())
        } else {
            p["targets"].as_array().is_some_and(|ts| {
                ts.iter().any(|t| {
                    t["target"] == "x"
                        && ["published", "confirmed_by_user"]
                            .contains(&t["status"].as_str().unwrap_or(""))
                })
            })
        }
    });
    ensure!(
        confirmed,
        "Destinations are not all confirmed. Recheck the website and record any verified X URL before reconciling; nothing was resent."
    );
    job.status = "completed".into();
    job.message =
        "Reconciled against destination receipts; user-confirmed URLs retain that evidence level."
            .into();
    save(root, &q)?;
    list(root)
}

pub(crate) fn parse_time(args: &Value) -> Result<DateTime<Utc>> {
    let timezone = args["timezone"].as_str().unwrap_or("UTC");
    let zone: chrono_tz::Tz = timezone.parse().context("Unknown timezone")?;
    let at = if let Some(local) = args["local_time"].as_str() {
        use chrono::TimeZone;
        let time = chrono::NaiveDateTime::parse_from_str(local, "%Y-%m-%d %H:%M")
            .context("Use YYYY-MM-DD HH:MM")?;
        zone.from_local_datetime(&time).single().context("This local time is ambiguous or does not exist because clocks change; choose another time or use an explicit UTC offset")?.with_timezone(&Utc)
    } else {
        DateTime::parse_from_rfc3339(
            args["at"]
                .as_str()
                .context("Use an ISO date with UTC offset")?,
        )?
        .with_timezone(&Utc)
    };
    Ok(at)
}
