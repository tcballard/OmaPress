use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use omapress_core::{
    preview,
    protocol::{self, Request},
};
use serde_json::{Value, json};
use std::{
    io::{self, Read},
    path::PathBuf,
};
#[derive(Parser)]
#[command(version, about = "Local-first publishing for Omarchy")]
struct Cli {
    #[command(subcommand)]
    command: Command,
    #[arg(long, global = true)]
    json: bool,
}
#[derive(Subcommand)]
enum Command {
    Init {
        path: PathBuf,
        #[arg(long, default_value = "My publication")]
        name: String,
        #[arg(long, default_value = "https://example.invalid")]
        base_url: String,
        #[arg(long, default_value = "Author")]
        author: String,
    },
    Inspect {
        path: PathBuf,
    },
    Validate {
        path: PathBuf,
    },
    Build {
        path: PathBuf,
        #[arg(long, default_value = "output")]
        output: PathBuf,
        #[arg(long)]
        expected_source_hash: String,
    },
    Preview {
        path: PathBuf,
        #[arg(long, default_value = "127.0.0.1")]
        bind: String,
        #[arg(long, default_value_t = 0)]
        port: u16,
        #[arg(long)]
        drafts: bool,
    },
    PublishPlan {
        path: PathBuf,
        #[arg(long)]
        expected_source_hash: String,
    },
    Publish {
        path: PathBuf,
        #[arg(long)]
        expected_source_hash: String,
        #[arg(long)]
        expected_remote_head: String,
    },
    Recheck {
        path: PathBuf,
    },
    Rollback {
        path: PathBuf,
        deployment_id: String,
        #[arg(long)]
        expected_source_hash: String,
        #[arg(long)]
        expected_remote_head: String,
    },
    ExportX {
        article: PathBuf,
        #[arg(long,default_value="html",value_parser=["html","text","caption","json"])]
        format: String,
    },
    FeedCheck {
        path: PathBuf,
    },
    /// Execute one versioned JSON request from stdin. All editor and agent operations use this protocol.
    Rpc,
}
fn execute(command: Command) -> Result<Value> {
    let (path, command, args) = match command {
        Command::Rpc => {
            let mut input = String::new();
            io::stdin()
                .take(6 * 1024 * 1024 + 1)
                .read_to_string(&mut input)?;
            ensure!(input.len() <= 6 * 1024 * 1024, "Request exceeds 6 MiB");
            return protocol::execute(serde_json::from_str(&input)?);
        }
        Command::Init {
            path,
            name,
            base_url,
            author,
        } => (
            path,
            "init",
            json!({"name":name,"base_url":base_url,"author":author}),
        ),
        Command::Inspect { path } => (path, "inspect", json!({})),
        Command::Validate { path } => (path, "validate", json!({})),
        Command::Build {
            path,
            output,
            expected_source_hash,
        } => (
            path,
            "build",
            json!({"output":output,"expected_source_hash":expected_source_hash}),
        ),
        Command::Preview {
            path,
            bind,
            port,
            drafts,
        } => {
            preview::run(&path, &bind, port, drafts)?;
            return Ok(Value::Null);
        }
        Command::PublishPlan {
            path,
            expected_source_hash,
        } => (
            path,
            "publish-plan",
            json!({"expected_source_hash":expected_source_hash}),
        ),
        Command::Publish {
            path,
            expected_source_hash,
            expected_remote_head,
        } => (
            path,
            "publish",
            json!({"expected_source_hash":expected_source_hash,"expected_remote_head":expected_remote_head}),
        ),
        Command::Recheck { path } => (path, "recheck", json!({})),
        Command::Rollback {
            path,
            deployment_id,
            expected_source_hash,
            expected_remote_head,
        } => (
            path,
            "rollback",
            json!({"deployment_id":deployment_id,"expected_source_hash":expected_source_hash,"expected_remote_head":expected_remote_head}),
        ),
        Command::FeedCheck { path } => (path, "feed-check", json!({})),
        Command::ExportX { article, format } => {
            let article = article.canonicalize()?;
            let root = article
                .ancestors()
                .find(|p| p.join("publication.toml").is_file())
                .context("Cannot find publication.toml above article")?;
            let result = protocol::execute(Request {
                schema: 1,
                command: "export-x".into(),
                path: root.into(),
                args: json!({"article":article.strip_prefix(root)?}),
            })?;
            if format == "json" {
                return Ok(result);
            }
            print!("{}", result[&format].as_str().unwrap_or(""));
            return Ok(Value::Null);
        }
    };
    protocol::execute(Request {
        schema: 1,
        path,
        command: command.into(),
        args,
    })
}
fn main() {
    let cli = Cli::parse();
    let silent = matches!(&cli.command,Command::ExportX{format,..}if format!="json");
    let response = protocol::response(execute(cli.command));
    let valid = response
        .result
        .as_ref()
        .and_then(|v| v.get("valid"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let ok = response.ok && valid;
    if !(silent && ok) {
        println!(
            "{}",
            serde_json::to_string(&response).unwrap_or_else(|_| {
                "{\"schema\":1,\"ok\":false,\"error\":\"Serialization failed\"}".into()
            })
        );
    }
    if !ok {
        std::process::exit(1);
    }
}
