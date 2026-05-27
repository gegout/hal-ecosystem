// Copyright (c) 2026 Cedric Gegout
// Licensed under the MIT License

use jira_news::config;
use jira_news::gemini;
use jira_news::jira;
use jira_news::protocol;

use anyhow::{anyhow, Context, Result};
use tracing::{error, info, warn};
use std::fs;
use std::io::BufRead;

#[tokio::main]
async fn main() {
    // Configure logging to both daily rolling file and stderr
    if let Err(e) = jira_news::logging::init_logger() {
        eprintln!("Failed to initialize logger: {:?}", e);
    }

    info!("Starting jira-news specialized application");

    // Read single JSON request from stdin
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut line = String::new();

    if let Err(e) = reader.read_line(&mut line) {
        error!("Failed to read line from stdin: {}", e);
        std::process::exit(1);
    }

    let request: protocol::ApplicationRequest = match serde_json::from_str(&line) {
        Ok(req) => req,
        Err(e) => {
            error!("Failed to parse request JSON: {}", e);
            std::process::exit(1);
        }
    };

    info!("Received request: {:?}", request);

    if let Err(e) = handle_request(&request).await {
        error!("Error handling request: {:?}", e);
        protocol::send_error(
            &request.request_id,
            "Failed to collect and summarize JIRA updates",
            &e.to_string(),
            "Verify config.toml, JIRA connectivity, and Gemini API keys",
        );
        std::process::exit(1);
    }
}

async fn handle_request(req: &protocol::ApplicationRequest) -> Result<()> {
    let jql = match req.command.as_str() {
        "jira6h" => "updated >= \"-6h\" ORDER BY updated DESC",
        "jira24h" => "updated >= \"-24h\" ORDER BY DESC", // Note: the prompt says 'updated >= "-24h" ORDER BY updated DESC'
        "jira48h" => "updated >= \"-48h\" ORDER BY updated DESC",
        "jirastatus48h" => "status CHANGED AFTER \"-48h\" ORDER BY updated DESC",
        other => {
            return Err(anyhow!("Unsupported command: {}", other));
        }
    };

    // Override exact command JQL mapping to be very safe and robust
    let jql = match req.command.as_str() {
        "jira6h" => "updated >= \"-6h\" ORDER BY updated DESC",
        "jira24h" => "updated >= \"-24h\" ORDER BY updated DESC",
        "jira48h" => "updated >= \"-48h\" ORDER BY updated DESC",
        "jirastatus48h" => "status CHANGED AFTER \"-48h\" ORDER BY updated DESC",
        _ => jql,
    };

    protocol::send_progress(&req.request_id, 10, "⚙️ Loading configuration...");

    let config = config::Config::load()
        .context("Failed to load configuration. Ensure ~/.config/jira-news/config.toml is correct.")?;

    protocol::send_progress(&req.request_id, 30, "📄 Connecting to JIRA...");

    let client = jira::JiraClient::new(&config.jira)?;
    
    protocol::send_progress(&req.request_id, 50, "📥 Querying JIRA issue updates...");
    
    info!("Executing JQL: {}", jql);
    let issues = client.search_issues(jql).await?;
    info!("Retrieved {} JIRA issues", issues.len());

    if issues.is_empty() {
        protocol::send_progress(&req.request_id, 95, "✅ Done.");
        protocol::send_final(
            &req.request_id,
            format!(
                "<b>📄 JIRA Updates</b>\n\nNo issue updates were found matching the JQL query: <code>{}</code>",
                jql
            ),
        );
        return Ok(());
    }

    protocol::send_progress(&req.request_id, 70, "📝 Formatting issues for summary...");

    let mut jira_md = String::new();
    jira_md.push_str(&format!(
        "JIRA issue updates from command: {} (JQL: {}):\n\n",
        req.command, jql
    ));

    for issue in &issues {
        jira_md.push_str(&format!("## Issue: {} ({})\n", issue.key, issue.fields.summary));
        jira_md.push_str(&format!("- **Status**: {}\n", issue.fields.status.name));
        let assignee_name = issue.fields.assignee.as_ref()
            .map(|u| u.display_name.as_str())
            .unwrap_or("Unassigned");
        jira_md.push_str(&format!("- **Assignee**: {}\n", assignee_name));
        jira_md.push_str(&format!("- **Updated**: {}\n", issue.fields.updated));
        if let Some(desc) = &issue.fields.description {
            let desc_str = extract_adf_text(desc);
            let truncated_desc = if desc_str.len() > 1000 {
                format!("{}... (truncated)", &desc_str[..1000])
            } else {
                desc_str
            };
            jira_md.push_str(&format!("- **Description**: {}\n", truncated_desc));
        } else {
            jira_md.push_str("- **Description**: None\n");
        }

        if let Some(comment_page) = &issue.fields.comment {
            if !comment_page.comments.is_empty() {
                jira_md.push_str("- **Comments**:\n");
                for comment in &comment_page.comments {
                    let comment_body = extract_adf_text(&comment.body);
                    jira_md.push_str(&format!(
                        "  * **{}** ({}): {}\n",
                        comment.author.display_name, comment.updated, comment_body
                    ));
                }
            }
        }
        jira_md.push_str("\n");
    }

    protocol::send_progress(&req.request_id, 80, "🧠 Summarizing with Gemini...");

    // Write the compact markdown to a temporary/cache file on disk before sending to Gemini
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| dirs::home_dir().map(|h| h.join(".cache")).unwrap_or_else(|| std::env::temp_dir()))
        .join("jira-news");
    fs::create_dir_all(&cache_dir).context("Failed to create cache directory for markdown input")?;
    let md_file_path = cache_dir.join("jira_updates.md");
    fs::write(&md_file_path, &jira_md).context("Failed to write compact markdown file")?;
    info!("Saved compact JIRA updates to markdown file: {:?}", md_file_path);

    // Read the compact markdown file to send its contents to Gemini
    let jira_md_from_file = fs::read_to_string(&md_file_path)
        .context("Failed to read compact JIRA updates from markdown file")?;

    let prompt_path = dirs::config_dir()
        .context("Could not determine config directory")?
        .join("jira-news")
        .join("jira-news_prompt.md");

    let system_prompt = if prompt_path.exists() {
        fs::read_to_string(&prompt_path)
            .with_context(|| format!("Failed to read prompt file from {:?}", prompt_path))?
    } else {
        return Err(anyhow!("System prompt file not found at {:?}", prompt_path));
    };

    let summary = match gemini::generate_content(&config.gemini, &system_prompt, &jira_md_from_file).await {
        Ok(s) => s,
        Err(e) => {
            warn!("Gemini generation completely failed: {:?}", e);
            "<b>⚠️ Gemini API Unavailable</b>\n\nWe apologize, but Gemini was not available to process this request at the moment. Please try again in a few minutes.".to_string()
        }
    };

    protocol::send_progress(&req.request_id, 95, "✅ Done.");
    protocol::send_final(&req.request_id, summary);

    Ok(())
}

fn extract_adf_text(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(obj) => {
            if let Some(serde_json::Value::String(t)) = obj.get("text") {
                t.clone()
            } else if let Some(serde_json::Value::Array(content)) = obj.get("content") {
                content.iter().map(extract_adf_text).collect::<Vec<_>>().join(" ")
            } else {
                String::new()
            }
        }
        serde_json::Value::Array(arr) => {
            arr.iter().map(extract_adf_text).collect::<Vec<_>>().join(" ")
        }
        _ => String::new(),
    }
}
