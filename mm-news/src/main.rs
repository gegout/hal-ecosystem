// Copyright (c) 2026 Cedric Gegout
// Licensed under the MIT License

use mm_news::config;
use mm_news::gemini;
use mm_news::mattermost;
use mm_news::models;
use mm_news::protocol;

use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::io::BufRead;
use tracing::{error, info, warn};
use std::fs;

#[tokio::main]
async fn main() {
    if let Err(e) = mm_news::logging::init_logger() {
        eprintln!("Failed to initialize logger: {:?}", e);
    }

    info!("Starting mm-news specialized application");

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
            "Failed to collect and summarize Mattermost news",
            &e.to_string(),
            "Verify configuration and Gemini/Mattermost connectivity",
        );
        std::process::exit(1);
    }
}

async fn handle_request(req: &protocol::ApplicationRequest) -> Result<()> {
    let lookback_hours = match req.command.as_str() {
        "mm6h" => 6,
        "mm24h" => 24,
        "mm48h" => 48,
        other => {
            return Err(anyhow!("Unsupported command: {}", other));
        }
    };

    protocol::send_progress(&req.request_id, 10, "⚙️ Loading configuration...");

    let config = config::Config::load()
        .context("Failed to load configuration. Ensure ~/.config/mm-news/config.toml is correct.")?;

    protocol::send_progress(&req.request_id, 30, "📄 Connecting to Mattermost...");

    let client = mattermost::MattermostClient::new(&config.mattermost)?;
    let channels = client.get_my_channels().await?;

    info!("Found {} joined channels", channels.len());

    let now = chrono::Utc::now();
    let since = now - chrono::Duration::hours(lookback_hours);
    let since_ms = since.timestamp_millis();

    protocol::send_progress(&req.request_id, 50, "📥 Retrieving latest messages...");

    let mut all_posts = Vec::new();
    let mut user_ids_to_resolve = std::collections::HashSet::new();
    let mut skipped_channels = 0;

    for channel in &channels {
        if let Some(last_post_time) = channel.last_post_at {
            if last_post_time < since_ms {
                skipped_channels += 1;
                continue;
            }
        }

        info!("Fetching posts for channel: {} ({})", channel.display_name, channel.name);
        match client.get_channel_posts(&channel.id, since_ms).await {
            Ok(post_list) => {
                // The API can return posts in any order. Let's filter active posts.
                for post_id in &post_list.order {
                    if let Some(post) = post_list.posts.get(post_id) {
                        if post.delete_at == 0 {
                            all_posts.push((channel.clone(), post.clone()));
                            user_ids_to_resolve.insert(post.user_id.clone());
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Failed to fetch posts for channel {}: {}. Skipping.", channel.display_name, e);
            }
        }
    }

    info!(
        "Processed messages. Fetch skipped for {} channels with no recent activity.",
        skipped_channels
    );

    protocol::send_progress(&req.request_id, 70, "👤 Resolving usernames...");

    // Resolve user profiles to get real usernames
    let user_ids: Vec<String> = user_ids_to_resolve.into_iter().collect();
    let resolved_users = match client.get_users_by_ids(&user_ids).await {
        Ok(users) => users,
        Err(e) => {
            warn!("Failed to resolve user IDs: {}. Using IDs instead.", e);
            vec![]
        }
    };

    let user_map: HashMap<String, String> = resolved_users
        .into_iter()
        .map(|u| (u.id, u.username))
        .collect();

    // Format chat log into markdown representation
    let mut chat_log_md = String::new();
    chat_log_md.push_str(&format!(
        "Mattermost messages from the last {} hours:\n\n",
        lookback_hours
    ));

    // Group by channel
    let mut channel_groups: HashMap<String, Vec<models::Post>> = HashMap::new();
    let mut channel_names = HashMap::new();

    for (channel, post) in all_posts {
        channel_names.insert(channel.id.clone(), channel.display_name.clone());
        channel_groups.entry(channel.id).or_default().push(post);
    }

    for (channel_id, mut posts) in channel_groups {
        let ch_name = channel_names.get(&channel_id).cloned().unwrap_or_default();
        chat_log_md.push_str(&format!("## Channel: {}\n", ch_name));

        // Sort posts by creation time ascending
        posts.sort_by_key(|p| p.create_at);

        for post in posts {
            let author = user_map.get(&post.user_id).cloned().unwrap_or_else(|| post.user_id.clone());
            chat_log_md.push_str(&format!("- **{}**: {}\n", author, post.message));
        }
        chat_log_md.push_str("\n");
    }

    if chat_log_md.trim().is_empty() || channel_names.is_empty() {
        protocol::send_progress(&req.request_id, 95, "✅ Done.");
        protocol::send_final(
            &req.request_id,
            format!(
                "<b>📄 Mattermost News</b>\n\nNo messages were found in the last {} hours.",
                lookback_hours
            ),
        );
        return Ok(());
    }

    protocol::send_progress(&req.request_id, 80, "🧠 Summarizing with Gemini...");

    let prompt_path = dirs::config_dir()
        .context("Could not determine config directory")?
        .join("mm-news")
        .join("mm-news_prompt.md");

    let system_prompt = if prompt_path.exists() {
        fs::read_to_string(&prompt_path)
            .with_context(|| format!("Failed to read prompt file from {:?}", prompt_path))?
    } else {
        return Err(anyhow!("System prompt file not found at {:?}", prompt_path));
    };

    let summary = match gemini::generate_content(&config.gemini, &system_prompt, &chat_log_md).await {
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
