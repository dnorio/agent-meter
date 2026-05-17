use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::Value;
use reqwest::Client;

const DEFAULT_COLLECTOR_URL: &str = "http://localhost:8081";

#[derive(Parser)]
#[command(name = "agent-meter", about = "Lightweight observability for agentic AI workflows")]
struct Cli {
    /// Collector base URL
    #[arg(long = "collector", default_value = DEFAULT_COLLECTOR_URL, env = "AGENT_METER_COLLECTOR_URL")]
    collector: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage tasks (start/end)
    Task {
        #[command(subcommand)]
        action: TaskAction,
    },
    /// Record events
    Event {
        #[command(subcommand)]
        action: EventAction,
    },
    /// Query reports
    Report {
        #[command(subcommand)]
        action: ReportAction,
    },
}

#[derive(Subcommand)]
enum TaskAction {
    /// Start a new task
    Start {
        /// Task identifier (e.g. TASK-001)
        task_id: String,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        branch: Option<String>,
        #[arg(long)]
        ide: Option<String>,
        #[arg(long)]
        agent: Option<String>,
        #[arg(long)]
        skill: Option<String>,
    },
    /// End an active task
    End {
        /// Task identifier
        task_id: String,
    },
    /// List recent tasks
    List {
        /// Max results
        #[arg(long, default_value = "20")]
        limit: i64,
    },
}

#[derive(Subcommand)]
enum EventAction {
    /// Record a tool call event
    ToolCall {
        #[arg(long)]
        task_id: Option<String>,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        branch: Option<String>,
        #[arg(long)]
        ide: Option<String>,
        #[arg(long)]
        agent: Option<String>,
        #[arg(long)]
        skill: Option<String>,
        #[arg(long = "mcp-server")]
        mcp_server: Option<String>,
        #[arg(long = "tool-name")]
        tool_name: String,
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        ok: bool,
        #[arg(long)]
        error: Option<String>,
        #[arg(long = "request-bytes")]
        request_bytes: Option<i32>,
        #[arg(long = "response-bytes")]
        response_bytes: Option<i32>,
        #[arg(long = "request-hash")]
        request_sha256: Option<String>,
        #[arg(long = "response-hash")]
        response_sha256: Option<String>,
        #[arg(long = "started-at")]
        started_at: Option<String>,
        #[arg(long = "ended-at")]
        ended_at: Option<String>,
    },
}

#[derive(Subcommand)]
enum ReportAction {
    /// Top tools by usage
    TopTools {
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        ide: Option<String>,
        #[arg(long)]
        skill: Option<String>,
        #[arg(long, default_value = "10")]
        limit: i64,
    },
    /// Top tasks by cost
    TopTasks {
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        ide: Option<String>,
        #[arg(long)]
        skill: Option<String>,
        #[arg(long, default_value = "10")]
        limit: i64,
    },
    /// Top MCP servers by usage
    TopMcpServers {
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        ide: Option<String>,
        #[arg(long)]
        skill: Option<String>,
        #[arg(long, default_value = "10")]
        limit: i64,
    },
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

impl Cli {
    async fn run(&self) -> Result<()> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        match &self.command {
            Commands::Task { action } => self.run_task(&client, action).await,
            Commands::Event { action } => self.run_event(&client, action).await,
            Commands::Report { action } => self.run_report(&client, action).await,
        }
    }

    async fn run_task(&self, client: &Client, action: &TaskAction) -> Result<()> {
        match action {
            TaskAction::Start { task_id, repo, branch, ide, agent, skill } => {
                let body = serde_json::json!({
                    "task_id": task_id,
                    "repo": repo,
                    "branch": branch,
                    "ide": ide,
                    "agent": agent,
                    "skill": skill,
                });
                let resp = client
                    .post(format!("{}/tasks/start", self.collector))
                    .json(&body)
                    .send()
                    .await?;
                let status = resp.status();
                let json: Value = resp.json().await?;
                if status.is_success() {
                    println!("task started: {} ({})", json["task_id"], json["started_at"]);
                } else {
                    eprintln!("error: {} - {}", status, serde_json::to_string(&json)?);
                }
            }
            TaskAction::End { task_id } => {
                let body = serde_json::json!({ "task_id": task_id });
                let resp = client
                    .post(format!("{}/tasks/end", self.collector))
                    .json(&body)
                    .send()
                    .await?;
                let status = resp.status();
                let json: Value = resp.json().await?;
                if status.is_success() {
                    println!("task ended: {} ({})", json["task_id"], json["ended_at"]);
                } else {
                    eprintln!("error: {} - {}", status, serde_json::to_string(&json)?);
                }
            }
            TaskAction::List { limit } => {
                let resp = client
                    .get(format!("{}/tasks?limit={}", self.collector, limit))
                    .send()
                    .await?;
                let json: Value = resp.json().await?;
                if let Some(arr) = json.as_array() {
                    if arr.is_empty() {
                        println!("no tasks found");
                    } else {
                        println!("{:<8} {:<20} {:<10} {:<10}", "ID", "TASK_ID", "STATUS", "DURATION");
                        println!("{}", "-".repeat(50));
                        for t in arr {
                            let status = if t["ended_at"].is_null() { "active" } else { "done" };
                            let dur = t["duration_secs"].as_i64().map(|d| format!("{}s", d)).unwrap_or_else(|| "-".into());
                            println!(
                                "{:<8} {:<20} {:<10} {:<10}",
                                t["id"].as_i64().unwrap_or(0),
                                t["task_id"].as_str().unwrap_or("?"),
                                status,
                                dur,
                            );
                        }
                    }
                } else {
                    println!("{}", serde_json::to_string_pretty(&json)?);
                }
            }
        }
        Ok(())
    }

    async fn run_event(&self, client: &Client, action: &EventAction) -> Result<()> {
        match action {
            EventAction::ToolCall {
                task_id,
                repo,
                branch,
                ide,
                agent,
                skill,
                mcp_server,
                tool_name,
                ok,
                error,
                request_bytes,
                response_bytes,
                request_sha256,
                response_sha256,
                started_at,
                ended_at,
            } => {
                let started = started_at.clone().unwrap_or_else(now_rfc3339);
                let ended = ended_at.clone().unwrap_or_else(now_rfc3339);
                let body = serde_json::json!({
                    "task_id": task_id,
                    "repo": repo,
                    "branch": branch,
                    "ide": ide,
                    "agent": agent,
                    "skill": skill,
                    "mcp_server": mcp_server,
                    "tool_name": tool_name,
                    "ok": ok,
                    "error": error,
                    "request_bytes": request_bytes,
                    "response_bytes": response_bytes,
                    "request_sha256": request_sha256,
                    "response_sha256": response_sha256,
                    "started_at": started,
                    "ended_at": ended,
                });
                let resp = client
                    .post(format!("{}/events/tool-call", self.collector))
                    .json(&body)
                    .send()
                    .await?;
                let status = resp.status();
                let json: Value = resp.json().await?;
                if status.is_success() {
                    println!(
                        "event recorded: {} | tokens: {} | duration: {}ms",
                        json["event_id"],
                        json["estimated_total_tokens"].as_i64().unwrap_or(0),
                        json["duration_ms"].as_i64().unwrap_or(0),
                    );
                } else {
                    eprintln!("error: {} - {}", status, serde_json::to_string(&json)?);
                }
            }
        }
        Ok(())
    }

    async fn run_report(&self, client: &Client, action: &ReportAction) -> Result<()> {
        let (path, pretty_title, columns) = match action {
            ReportAction::TopTools { from, to, repo, ide, skill, limit } => {
                let params = self.build_report_params(from, to, repo, ide, skill, *limit);
                (format!("/reports/top-tools?{}", params), "TOP TOOLS", vec!["MCP Server", "Tool", "Calls", "Tokens", "Avg (ms)", "Errors"])
            }
            ReportAction::TopTasks { from, to, repo, ide, skill, limit } => {
                let params = self.build_report_params(from, to, repo, ide, skill, *limit);
                (format!("/reports/top-tasks?{}", params), "TOP TASKS", vec!["Task ID", "Calls", "Tokens", "Duration", "Errors", "Tools"])
            }
            ReportAction::TopMcpServers { from, to, repo, ide, skill, limit } => {
                let params = self.build_report_params(from, to, repo, ide, skill, *limit);
                (format!("/reports/top-mcp-servers?{}", params), "TOP MCP SERVERS", vec!["Server", "Calls", "Tokens", "Avg Resp", "Err Rate"])
            }
        };

        let url = format!("{}{}", self.collector, path);
        let resp = client.get(&url).send().await?;
        let json: Value = resp.json().await?;

        println!("\n  ╔══════════════════════════════════════════╗");
        println!("  ║  {:<38} ║", pretty_title);
        println!("  ╚══════════════════════════════════════════╝\n");

        if let Some(arr) = json.as_array() {
            if arr.is_empty() {
                println!("  no data found for this period");
            } else {
                for col in &columns {
                    print!("  {:<20}", col);
                }
                println!();
                println!("  {}", "─".repeat(columns.len() * 22));
                for row in arr {
                    match pretty_title {
                        "TOP TOOLS" => {
                            let server = row["mcp_server"].as_str().unwrap_or("-").to_string();
                            let tool = row["tool_name"].as_str().unwrap_or("?");
                            let calls = row["calls"].as_i64().unwrap_or(0);
                            let tokens = row["total_estimated_tokens"].as_i64().map(|t| format_tokens(t)).unwrap_or_else(|| "-".into());
                            let avg = row["avg_duration_ms"].as_f64().map(|d| format!("{:.0}", d)).unwrap_or_else(|| "-".into());
                            let errs = row["errors"].as_i64().unwrap_or(0);
                            println!("  {:<20} {:<20} {:<20} {:<20} {:<20} {:<20}", truncate(server, 18), truncate(tool.into(), 18), calls, tokens, avg, errs);
                        }
                        "TOP TASKS" => {
                            let tid = row["task_id"].as_str().unwrap_or("?");
                            let calls = row["tool_calls"].as_i64().unwrap_or(0);
                            let tokens = row["total_estimated_tokens"].as_i64().map(|t| format_tokens(t)).unwrap_or_else(|| "-".into());
                            let dur = row["total_duration_ms"].as_i64().map(|d| format_duration(d)).unwrap_or_else(|| "-".into());
                            let errs = row["errors"].as_i64().unwrap_or(0);
                            let tools = row["distinct_tools"].as_i64().unwrap_or(0);
                            println!("  {:<20} {:<20} {:<20} {:<20} {:<20} {:<20}", truncate(tid.into(), 18), calls, tokens, dur, errs, tools);
                        }
                        _ => {
                            let server = row["mcp_server"].as_str().unwrap_or("?");
                            let calls = row["calls"].as_i64().unwrap_or(0);
                            let tokens = row["total_estimated_tokens"].as_i64().map(|t| format_tokens(t)).unwrap_or_else(|| "-".into());
                            let avg_r = row["avg_response_bytes"].as_f64().map(|b| format!("{:.0}", b)).unwrap_or_else(|| "-".into());
                            let err_rate = row["error_rate"].as_f64().map(|r| format!("{:.1}%", r * 100.0)).unwrap_or_else(|| "-".into());
                            println!("  {:<20} {:<20} {:<20} {:<20} {:<20}", truncate(server.into(), 18), calls, tokens, avg_r, err_rate);
                        }
                    }
                }
            }
        } else {
            println!("  {}", serde_json::to_string_pretty(&json)?);
        }
        println!();
        Ok(())
    }

    fn build_report_params(
        &self,
        from: &Option<String>,
        to: &Option<String>,
        repo: &Option<String>,
        ide: &Option<String>,
        skill: &Option<String>,
        limit: i64,
    ) -> String {
        let mut parts: Vec<String> = vec![format!("limit={}", limit)];
        if let Some(v) = from { parts.push(format!("from={}", v)); }
        if let Some(v) = to { parts.push(format!("to={}", v)); }
        if let Some(v) = repo { parts.push(format!("repo={}", v)); }
        if let Some(v) = ide { parts.push(format!("ide={}", v)); }
        if let Some(v) = skill { parts.push(format!("skill={}", v)); }
        parts.join("&")
    }
}

fn format_tokens(t: i64) -> String {
    if t >= 1_000_000 {
        format!("{:.1}M", t as f64 / 1_000_000.0)
    } else if t >= 1_000 {
        format!("{:.1}k", t as f64 / 1_000.0)
    } else {
        format!("{}", t)
    }
}

fn format_duration(ms: i64) -> String {
    if ms >= 60_000 {
        format!("{:.1}min", ms as f64 / 60_000.0)
    } else {
        format!("{}ms", ms)
    }
}

fn truncate(s: String, max: usize) -> String {
    if s.len() > max {
        format!("{}…", &s[..max])
    } else {
        s
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Err(e) = cli.run().await {
        eprintln!("error: {:#}", e);
        std::process::exit(1);
    }
    Ok(())
}
