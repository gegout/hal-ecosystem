// Copyright (c) 2026 Cedric Gegout
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::ParseMode;
use tracing::{error, info, warn};

use crate::protocol::ApplicationResponse;
use crate::router::Router;
use crate::session::SessionManager;
use crate::telemetry::{TelemetryManager, TelemetryRecord};

pub async fn start_bot(
    bot_token: String,
    router: Arc<Router>,
    session_manager: Arc<SessionManager>,
    telemetry_manager: Arc<TelemetryManager>,
) -> Result<(), anyhow::Error> {
    info!("Initializing Telegram frontend");
    let bot = Bot::new(bot_token);

    let handler = dptree::entry()
        .branch(
            Update::filter_message().endpoint(
                move |bot: Bot,
                      msg: Message,
                      router: Arc<Router>,
                      session_manager: Arc<SessionManager>,
                      telemetry_manager: Arc<TelemetryManager>| async move {
                    handle_message(bot, msg, router, session_manager, telemetry_manager).await
                },
            ),
        );

    info!("Starting HAL Telegram listener loop");
    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![
            router,
            session_manager,
            telemetry_manager
        ])
        .build()
        .dispatch()
        .await;

    Ok(())
}

async fn handle_message(
    bot: Bot,
    msg: Message,
    router: Arc<Router>,
    session_manager: Arc<SessionManager>,
    telemetry_manager: Arc<TelemetryManager>,
) -> ResponseResult<()> {
    let user_id = match msg.from() {
        Some(u) => u.id.0 as i64,
        None => return Ok(()), // Ignore anonymous/channel posts
    };
    let chat_id = msg.chat.id.0;

    // 1. Authorization check
    if !router.is_authorized(user_id).await {
        warn!("Dropped message from unauthorized user_id={}", user_id);
        return Ok(());
    }

    let text = match msg.text() {
        Some(t) => t.to_string(),
        None => return Ok(()), // Ignore non-text messages
    };

    info!("Parsing Telegram message from user_id={}", user_id);

    // 2. Initialize Session
    let _session = session_manager.add_message(chat_id, user_id, "user", &text);

    // 3. Send initial progress message
    let initial_text = crate::progress::format_progress_message(0, "Initializing request...");
    let progress_msg = match bot
        .send_message(msg.chat.id, initial_text)
        .parse_mode(ParseMode::Html)
        .await
    {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to send initial progress message to Telegram: {}", e);
            return Ok(());
        }
    };

    // 4. Create communication pipe
    let (tx, mut rx) = tokio::sync::mpsc::channel::<ApplicationResponse>(32);

    // 5. Spawn router worker
    let router_clone = router.clone();
    let text_clone = text.clone();
    let route_worker = tokio::spawn(async move {
        let start_time = std::time::Instant::now();
        let res = router_clone.route(&text_clone, user_id, chat_id, tx).await;
        let latency_ms = start_time.elapsed().as_millis();
        (res, latency_ms)
    });

    // 6. Monitor progress updates
    let bot_clone = bot.clone();
    let chat_id_obj = msg.chat.id;
    let msg_id = progress_msg.id;

    while let Some(response) = rx.recv().await {
        if let ApplicationResponse::Progress(update) = response {
            info!("Receiving progress update: {}%", update.percent);
            info!("Updating Telegram progress message");
            let progress_html = crate::progress::format_progress_message(update.percent, &update.message);
            
            if let Err(e) = bot_clone
                .edit_message_text(chat_id_obj, msg_id, progress_html)
                .parse_mode(ParseMode::Html)
                .await
            {
                let err_str = e.to_string();
                if !err_str.contains("message is not modified") {
                    error!("Failed to edit progress update: {}", err_str);
                }
            }
        }
    }

    // 7. Await worker completion
    let (outcome, latency_ms) = match route_worker.await {
        Ok(result) => result,
        Err(e) => (Err(anyhow::anyhow!("Task join error: {}", e)), 0),
    };

    let mut command_name = "unknown".to_string();
    if let Some(cmd) = text.trim().split_whitespace().next() {
        if cmd.starts_with('/') {
            command_name = cmd[1..].to_string();
        }
    }

    let success = outcome.is_ok();
    let mut error_reason = None;

    match outcome {
        Ok(final_resp) => {
            info!("Sending final Telegram response");
            let final_message = if final_resp.trusted_html == Some(true) {
                sanitize_telegram_html(&final_resp.message)
            } else {
                crate::progress::escape_html(&final_resp.message)
            };

            let _ = session_manager.add_message(chat_id, user_id, "bot", &final_message);

            if let Err(e) = bot
                .edit_message_text(chat_id_obj, msg_id, final_message)
                .parse_mode(ParseMode::Html)
                .await
            {
                let err_msg = e.to_string();
                error!("Failed to send final response to Telegram: {}", err_msg);
                
                if err_msg.contains("can't parse entities") {
                    let fallback_card = crate::progress::format_error_card(
                        "Telegram rejected the AI response format",
                        Some(&err_msg),
                        Some("The AI generated invalid HTML formatting. Please retry your command.")
                    );
                    
                    let _ = bot.edit_message_text(chat_id_obj, msg_id, fallback_card)
                        .parse_mode(ParseMode::Html)
                        .await;
                }
            }
        }
        Err(e) => {
            let err_reason_str = e.to_string();
            error!("Error handling request: {}", err_reason_str);
            error_reason = Some(err_reason_str.clone());

            let error_card = crate::progress::format_error_card(
                &err_reason_str,
                None,
                Some("Verify the command syntax or application status"),
            );

            let _ = session_manager.add_message(chat_id, user_id, "bot", &error_card);

            if let Err(err_edit) = bot
                .edit_message_text(chat_id_obj, msg_id, error_card)
                .parse_mode(ParseMode::Html)
                .await
            {
                error!("Failed to send error card to Telegram: {}", err_edit);
            }
        }
    }

    // 8. Record operational metrics
    telemetry_manager
        .record(TelemetryRecord {
            timestamp: chrono::Utc::now(),
            request_id: uuid::Uuid::new_v4().to_string(), // new telemetry log request_id
            command: command_name,
            application: "router".to_string(), // high level router
            user_id,
            chat_id,
            latency_ms,
            success,
            error_reason,
        })
        .await;

    Ok(())
}

pub fn sanitize_telegram_html(html: &str) -> String {
    // 1. Convert standard list item tags to clean bullets
    let step1 = html.replace("<li>", "• ");
    let step2 = step1.replace("</li>", "\n");
    let step3 = step2.replace("<ul>", "").replace("</ul>", "");
    let step4 = step3.replace("<ol>", "").replace("</ol>", "");
    
    // 2. Convert standard paragraph tags to double newlines
    let step5 = step4.replace("<p>", "").replace("</p>", "\n\n");
    
    // 3. Convert br tags to newlines
    let step6 = step5
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<BR>", "\n")
        .replace("<BR/>", "\n")
        .replace("<BR />", "\n");

    // 4. State machine to escape raw '<', '>', and '&' that are not part of allowed tags or valid entities
    let allowed_tags = [
        "b", "/b", "strong", "/strong",
        "i", "/i", "em", "/em",
        "u", "/u",
        "s", "/s", "strike", "/strike", "del", "/del",
        "code", "/code",
        "pre", "/pre",
        "tg-spoiler", "/tg-spoiler"
    ];

    let mut result = String::new();
    let mut chars = step6.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '<' {
            // Check if this is an allowed tag or 'a' href tag
            let mut tag_content = String::new();
            let mut is_tag = false;
            let mut matched_chars = vec!['<'];

            while let Some(&next_c) = chars.peek() {
                if next_c == '>' {
                    chars.next();
                    matched_chars.push('>');
                    is_tag = true;
                    break;
                } else if next_c == '<' {
                    break;
                } else {
                    tag_content.push(next_c);
                    matched_chars.push(next_c);
                    chars.next();
                }
            }

            if is_tag {
                let tag_name = tag_content.trim();
                let lower_tag = tag_name.to_lowercase();
                let is_br = lower_tag == "br"
                    || lower_tag == "br/"
                    || lower_tag == "/br"
                    || lower_tag.starts_with("br ")
                    || lower_tag.starts_with("br/")
                    || lower_tag.starts_with("/br");

                if is_br {
                    result.push('\n');
                } else {
                    let is_allowed = allowed_tags.contains(&lower_tag.as_str())
                        || lower_tag.starts_with("a ")
                        || lower_tag == "a"
                        || lower_tag == "/a";

                    if is_allowed {
                        for mc in matched_chars {
                            result.push(mc);
                        }
                    } else {
                        result.push_str("&lt;");
                        for &mc in &matched_chars[1..] {
                            if mc == '>' {
                                result.push_str("&gt;");
                            } else {
                                result.push(mc);
                            }
                        }
                    }
                }
            } else {
                result.push_str("&lt;");
                for &mc in &matched_chars[1..] {
                    result.push(mc);
                }
            }
        } else if c == '&' {
            let mut entity = String::new();
            let mut is_entity = false;
            let mut matched_chars = vec!['&'];

            while let Some(&next_c) = chars.peek() {
                if next_c == ';' {
                    chars.next();
                    matched_chars.push(';');
                    is_entity = true;
                    break;
                } else if next_c.is_alphanumeric() && entity.len() < 8 {
                    entity.push(next_c);
                    matched_chars.push(next_c);
                    chars.next();
                } else {
                    break;
                }
            }

            if is_entity && (entity == "lt" || entity == "gt" || entity == "amp" || entity == "quot" || entity == "apos") {
                for mc in matched_chars {
                    result.push(mc);
                }
            } else {
                result.push_str("&amp;");
                for &mc in &matched_chars[1..] {
                    result.push(mc);
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}
