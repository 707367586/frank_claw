//! ClawX CLI — interactive command-line interface for the agent runtime.

use anyhow::Result;
use clap::{Parser, Subcommand};
use clawx_controlplane_client::ControlPlaneClient;
use futures::StreamExt;
use serde_json::{json, Value};
use std::io::Write;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "clawx", about = "ClawX agent runtime CLI", version)]
struct Cli {
    /// TCP port for dev mode (default: use UDS)
    #[arg(long)]
    dev_port: Option<u16>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage agents
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },
    /// Start an interactive chat session
    Chat {
        /// Agent ID to chat with
        agent_id: String,
    },
    /// Manage long-term memories
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },
    /// Manage knowledge base
    Kb {
        #[command(subcommand)]
        action: KbAction,
    },
    /// Manage workspace vault
    Vault {
        #[command(subcommand)]
        action: VaultAction,
    },
    /// Manage LLM model providers
    Model {
        #[command(subcommand)]
        action: ModelAction,
    },
    /// System management
    System {
        #[command(subcommand)]
        action: SystemAction,
    },
    /// Manage the background service daemon
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
}

#[derive(Subcommand)]
enum AgentAction {
    List,
    Create { name: String },
    Show { id: String },
    Update { id: String },
    Delete { id: String },
    Clone { id: String },
}

#[derive(Subcommand)]
enum MemoryAction {
    List,
    Search { query: String },
    Stats,
}

#[derive(Subcommand)]
enum KbAction {
    Sources,
    Search { query: String },
}

#[derive(Subcommand)]
enum VaultAction {
    List,
}

#[derive(Subcommand)]
enum ModelAction {
    List,
    Show { id: String },
    Delete { id: String },
}

#[derive(Subcommand)]
enum SystemAction {
    Health,
}

#[derive(Subcommand)]
enum ServiceAction {
    /// Install the launchd plist to auto-start the service
    Install,
    /// Uninstall the launchd plist
    Uninstall,
    /// Show the generated plist content
    Show,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let client = build_client(cli.dev_port).await?;

    match cli.command {
        Commands::Agent { action } => handle_agent(&client, action).await?,
        Commands::Chat { agent_id } => handle_chat(&client, &agent_id).await?,
        Commands::Memory { action } => handle_memory(&client, action).await?,
        Commands::Kb { action } => handle_kb(&client, action).await?,
        Commands::Vault { action } => handle_vault(&client, action).await?,
        Commands::Model { action } => handle_model(&client, action).await?,
        Commands::System { action } => handle_system(&client, action).await?,
        Commands::Service { action } => handle_service(action).await?,
    }

    Ok(())
}

async fn build_client(dev_port: Option<u16>) -> Result<ControlPlaneClient> {
    let port = dev_port.unwrap_or(9800);
    let token = ControlPlaneClient::read_token("~/.clawx")
        .await
        .unwrap_or_default();
    Ok(ControlPlaneClient::new_tcp(port, token))
}

async fn handle_agent(client: &ControlPlaneClient, action: AgentAction) -> Result<()> {
    match action {
        AgentAction::List => {
            let agents: Vec<Value> = client.get("/agents").await?;
            if agents.is_empty() {
                println!("No agents found.");
            } else {
                for agent in agents {
                    println!(
                        "  {} | {} | {} | {}",
                        agent["id"].as_str().unwrap_or("-"),
                        agent["name"].as_str().unwrap_or("-"),
                        agent["role"].as_str().unwrap_or("-"),
                        agent["status"].as_str().unwrap_or("-"),
                    );
                }
            }
        }
        AgentAction::Create { name } => {
            let body = json!({
                "name": name,
                "role": "assistant",
                "model_id": uuid::Uuid::new_v4().to_string(),
            });
            let agent: Value = client.post("/agents", &body).await?;
            println!("Created agent: {}", agent["id"]);
        }
        AgentAction::Show { id } => {
            let agent: Value = client.get(&format!("/agents/{}", id)).await?;
            println!("{}", serde_json::to_string_pretty(&agent)?);
        }
        AgentAction::Update { id } => {
            // Minimal update — just touches updated_at
            let body = json!({});
            let agent: Value = client.put(&format!("/agents/{}", id), &body).await?;
            println!("Updated agent: {}", agent["name"]);
        }
        AgentAction::Delete { id } => {
            client.delete(&format!("/agents/{}", id)).await?;
            println!("Deleted agent {}", id);
        }
        AgentAction::Clone { id } => {
            let agent: Value = client.post(&format!("/agents/{}/clone", &id), &json!({})).await?;
            println!("Cloned agent: {} -> {}", id, agent["id"]);
        }
    }
    Ok(())
}

async fn handle_chat(client: &ControlPlaneClient, agent_id: &str) -> Result<()> {
    // Verify agent exists
    let agent: Value = client
        .get(&format!("/agents/{}", agent_id))
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "Agent '{}' not found. Use 'clawx agent list' to see available agents.",
                agent_id
            )
        })?;

    let agent_name = agent["name"].as_str().unwrap_or("Agent");
    println!(
        "Chat with {} ({}). Type 'exit' or Ctrl+C to quit.\n",
        agent_name, agent_id
    );

    // Create a conversation
    let conv: Value = client
        .post("/conversations", &json!({ "agent_id": agent_id }))
        .await?;
    let conv_id = conv["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("failed to create conversation"))?;

    // Interactive loop
    let stdin = std::io::stdin();
    loop {
        // Print prompt
        eprint!("You> ");
        std::io::stderr().flush().ok();

        let mut input = String::new();
        if stdin.read_line(&mut input).is_err() || input.is_empty() {
            break;
        }
        let input = input.trim();

        if input.is_empty() {
            continue;
        }
        if input == "exit" || input == "quit" {
            println!("Goodbye!");
            break;
        }

        // Send message via SSE streaming
        eprint!("\n{}> ", agent_name);
        std::io::stderr().flush().ok();

        let url = format!(
            "{}/conversations/{}/messages",
            client.base_url(),
            conv_id
        );
        let resp = client
            .http()
            .post(&url)
            .bearer_auth(client.token())
            .json(&json!({
                "role": "user",
                "content": input,
                "stream": true
            }))
            .send()
            .await;

        match resp {
            Ok(response) if response.status().is_success() => {
                let mut stream = response.bytes_stream();
                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(bytes) => {
                            let text = String::from_utf8_lossy(&bytes);
                            for line in text.lines() {
                                if let Some(data) = line.strip_prefix("data: ") {
                                    if data == "[DONE]" {
                                        break;
                                    }
                                    if let Ok(parsed) = serde_json::from_str::<Value>(data) {
                                        if let Some(content) = parsed["content"].as_str() {
                                            print!("{}", content);
                                            std::io::stdout().flush().ok();
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("\n[stream error: {}]", e);
                            break;
                        }
                    }
                }
                println!("\n");
            }
            Ok(response) => {
                eprintln!(
                    "\n[request error: server returned status {}]",
                    response.status()
                );
            }
            Err(e) => {
                eprintln!("\n[request error: {}]", e);
            }
        }
    }

    Ok(())
}

async fn handle_memory(client: &ControlPlaneClient, action: MemoryAction) -> Result<()> {
    match action {
        MemoryAction::List => {
            let result: Value = client.get("/memories?page=1&per_page=20").await?;
            let items = result["items"].as_array();
            match items {
                Some(items) if !items.is_empty() => {
                    for mem in items {
                        println!(
                            "  {} | {} | {} | imp={:.1}",
                            mem["id"].as_str().unwrap_or("-"),
                            mem["kind"].as_str().unwrap_or("-"),
                            mem["summary"].as_str().unwrap_or("-"),
                            mem["importance"].as_f64().unwrap_or(0.0),
                        );
                    }
                }
                _ => println!("No memories found."),
            }
        }
        MemoryAction::Search { query } => {
            let body = json!({ "query_text": query, "top_k": 5 });
            let results: Vec<Value> = client.post("/memories/search", &body).await?;
            if results.is_empty() {
                println!("No matching memories.");
            } else {
                for mem in results {
                    println!(
                        "  [{:.2}] {}",
                        mem["combined_score"].as_f64().unwrap_or(0.0),
                        mem["entry"]["summary"].as_str().unwrap_or("-"),
                    );
                }
            }
        }
        MemoryAction::Stats => {
            let stats: Value = client.get("/memories/stats").await?;
            println!("{}", serde_json::to_string_pretty(&stats)?);
        }
    }
    Ok(())
}

async fn handle_kb(client: &ControlPlaneClient, action: KbAction) -> Result<()> {
    match action {
        KbAction::Sources => {
            let sources: Vec<Value> = client.get("/knowledge/sources").await?;
            if sources.is_empty() {
                println!("No knowledge sources.");
            } else {
                for src in sources {
                    println!(
                        "  {} | {} | {} files",
                        src["id"].as_str().unwrap_or("-"),
                        src["path"].as_str().unwrap_or("-"),
                        src["file_count"].as_u64().unwrap_or(0),
                    );
                }
            }
        }
        KbAction::Search { query } => {
            let body = json!({ "query_text": query, "top_n": 5 });
            let results: Vec<Value> = client.post("/knowledge/search", &body).await?;
            if results.is_empty() {
                println!("No matching knowledge.");
            } else {
                for r in results {
                    println!(
                        "  [{}] {}",
                        r["document_path"].as_str().unwrap_or("-"),
                        &r["chunk"]["content"].as_str().unwrap_or("-")[..80.min(
                            r["chunk"]["content"].as_str().unwrap_or("").len()
                        )],
                    );
                }
            }
        }
    }
    Ok(())
}

async fn handle_vault(client: &ControlPlaneClient, action: VaultAction) -> Result<()> {
    match action {
        VaultAction::List => {
            let snapshots: Vec<Value> = client.get("/vault").await?;
            if snapshots.is_empty() {
                println!("No vault snapshots.");
            } else {
                for snap in snapshots {
                    println!(
                        "  {} | {} | {}",
                        snap["id"].as_str().unwrap_or("-"),
                        snap["label"].as_str().unwrap_or("-"),
                        snap["created_at"].as_str().unwrap_or("-"),
                    );
                }
            }
        }
    }
    Ok(())
}

async fn handle_model(client: &ControlPlaneClient, action: ModelAction) -> Result<()> {
    match action {
        ModelAction::List => {
            let models: Vec<Value> = client.get("/models").await?;
            if models.is_empty() {
                println!("No model providers configured.");
            } else {
                for m in models {
                    println!(
                        "  {} | {} | {} | {}",
                        m["id"].as_str().unwrap_or("-"),
                        m["name"].as_str().unwrap_or("-"),
                        m["provider_type"].as_str().unwrap_or("-"),
                        m["model_name"].as_str().unwrap_or("-"),
                    );
                }
            }
        }
        ModelAction::Show { id } => {
            let model: Value = client.get(&format!("/models/{}", id)).await?;
            println!("{}", serde_json::to_string_pretty(&model)?);
        }
        ModelAction::Delete { id } => {
            client.delete(&format!("/models/{}", id)).await?;
            println!("Deleted model provider {}", id);
        }
    }
    Ok(())
}

async fn handle_system(client: &ControlPlaneClient, action: SystemAction) -> Result<()> {
    match action {
        SystemAction::Health => {
            let health: Value = client.get("/system/health").await?;
            println!("{}", serde_json::to_string_pretty(&health)?);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Service (launchd) management
// ---------------------------------------------------------------------------

const LAUNCHD_LABEL: &str = "com.clawx.service";

fn plist_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("Library/LaunchAgents")
        .join(format!("{}.plist", LAUNCHD_LABEL))
}

fn generate_plist() -> String {
    // Find the clawx-service binary next to the CLI binary
    let service_bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("clawx-service")))
        .unwrap_or_else(|| std::path::PathBuf::from("/usr/local/bin/clawx-service"));

    let home = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .to_string_lossy()
        .to_string();

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{bin}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{home}/.clawx/logs/service.out.log</string>
    <key>StandardErrorPath</key>
    <string>{home}/.clawx/logs/service.err.log</string>
    <key>WorkingDirectory</key>
    <string>{home}</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>HOME</key>
        <string>{home}</string>
    </dict>
</dict>
</plist>
"#,
        label = LAUNCHD_LABEL,
        bin = service_bin.display(),
        home = home,
    )
}

async fn handle_service(action: ServiceAction) -> Result<()> {
    match action {
        ServiceAction::Show => {
            println!("{}", generate_plist());
        }
        ServiceAction::Install => {
            let path = plist_path();
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            // Also create logs directory
            let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
            tokio::fs::create_dir_all(home.join(".clawx/logs")).await?;

            let content = generate_plist();
            tokio::fs::write(&path, &content).await?;
            println!("Plist written to {}", path.display());

            // Load with launchctl
            let status = tokio::process::Command::new("launchctl")
                .args(["load", path.to_str().unwrap()])
                .status()
                .await?;

            if status.success() {
                println!("Service installed and started.");
            } else {
                eprintln!("launchctl load failed (exit {}). Try manually: launchctl load {}", status, path.display());
            }
        }
        ServiceAction::Uninstall => {
            let path = plist_path();
            if !path.exists() {
                println!("Plist not found at {}. Nothing to uninstall.", path.display());
                return Ok(());
            }

            let status = tokio::process::Command::new("launchctl")
                .args(["unload", path.to_str().unwrap()])
                .status()
                .await?;

            if status.success() {
                println!("Service stopped.");
            } else {
                eprintln!("launchctl unload failed (exit {}).", status);
            }

            tokio::fs::remove_file(&path).await?;
            println!("Plist removed: {}", path.display());
        }
    }
    Ok(())
}
