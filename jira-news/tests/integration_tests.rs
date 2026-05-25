// Copyright (c) 2026 Cedric Gegout
// Licensed under the MIT License

#[cfg(test)]
mod tests {
    use serde_json::json;
    use jira_news::protocol::{ApplicationRequest, ProgressUpdate, FinalResponse};
    use jira_news::config::{GeminiConfig, JiraConfig};
    use jira_news::gemini;
    use jira_news::jira::JiraClient;
    use tokio::net::TcpListener;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_protocol_serialization() {
        let req_id = "test-id-123";
        
        let progress = ProgressUpdate {
            r#type: "progress".to_string(),
            request_id: req_id.to_string(),
            percent: 50,
            message: "Testing...".to_string(),
            format: "html".to_string(),
        };

        let progress_json = serde_json::to_string(&progress).unwrap();
        let parsed_progress: serde_json::Value = serde_json::from_str(&progress_json).unwrap();
        assert_eq!(parsed_progress["type"], "progress");
        assert_eq!(parsed_progress["percent"], 50);

        let final_resp = FinalResponse {
            r#type: "final".to_string(),
            request_id: req_id.to_string(),
            format: "html".to_string(),
            message: "<b>Done</b>".to_string(),
            trusted_html: true,
        };

        let final_json = serde_json::to_string(&final_resp).unwrap();
        let parsed_final: serde_json::Value = serde_json::from_str(&final_json).unwrap();
        assert_eq!(parsed_final["type"], "final");
        assert_eq!(parsed_final["trusted_html"], true);
    }

    #[test]
    fn test_request_deserialization() {
        let incoming = json!({
            "request_id": "uuid-v4-string",
            "command": "jira6h",
            "arguments": "",
            "raw_message": "/jira6h",
            "user_id": 123456789,
            "chat_id": 987654321
        });

        let incoming_str = serde_json::to_string(&incoming).unwrap();
        let req: ApplicationRequest = serde_json::from_str(&incoming_str).unwrap();
        assert_eq!(req.request_id, "uuid-v4-string");
        assert_eq!(req.command, "jira6h");
        assert_eq!(req.chat_id, 987654321);
    }

    #[tokio::test]
    async fn test_gemini_fallback_mechanism() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mock_server_url = format!("http://{}", addr);

        let request_counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = request_counter.clone();

        tokio::spawn(async move {
            for _ in 0..3 {
                if let Ok((mut socket, _)) = listener.accept().await {
                    let mut buffer = [0; 1024];
                    let _ = socket.read(&mut buffer).await;

                    let count = counter_clone.fetch_add(1, Ordering::SeqCst);

                    let response = match count {
                        0 => {
                            "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n"
                        }
                        1 => {
                            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n"
                        }
                        2 => {
                            let success_json = json!({
                                "candidates": [{
                                    "content": {
                                        "parts": [{
                                            "text": "Mocked summary from fallback model"
                                        }]
                                    }
                                }]
                            });
                            let body = success_json.to_string();
                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                                body.len(),
                                body
                            );
                            Box::leak(resp.into_boxed_str())
                        }
                        _ => "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n",
                    };

                    let _ = socket.write_all(response.as_bytes()).await;
                    let _ = socket.flush().await;
                }
            }
        });

        let config = GeminiConfig {
            api_key: "test_key".to_string(),
            preferred_models: Some(vec![
                "gemini-3.5-flash".to_string(),
                "gemini-2.5-pro".to_string(),
                "gemini-2.5-flash".to_string(),
            ]),
            test_mock_url: Some(mock_server_url),
        };

        let res = gemini::generate_content(&config, "system prompt", "user prompt").await;
        
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), "Mocked summary from fallback model");
        assert_eq!(request_counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_jira_mock_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mock_server_url = format!("http://{}", addr);

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buffer = [0; 1024];
                let _ = socket.read(&mut buffer).await;

                let mock_response = json!({
                    "issues": [
                        {
                            "key": "CAN-101",
                            "fields": {
                                "summary": "Fix broken pipeline on focal",
                                "status": { "name": "In Progress" },
                                "assignee": { "displayName": "Alice Smith" },
                                "updated": "2026-05-25T14:00:00.000+0200",
                                "description": "Investigation shows focal has broken pip dependencies",
                                "comment": {
                                    "comments": [
                                        {
                                            "author": { "displayName": "Bob Jones" },
                                            "body": "Working on reproducing this now",
                                            "updated": "2026-05-25T14:30:00.000+0200"
                                        }
                                    ]
                                }
                            }
                        }
                    ]
                });

                let body = mock_response.to_string();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );

                let _ = socket.write_all(resp.as_bytes()).await;
                let _ = socket.flush().await;
            }
        });

        let config = JiraConfig {
            base_url: "http://dummy_url".to_string(),
            token: "test_token".to_string(),
            user_email: Some("test@example.com".to_string()),
            test_mock_url: Some(mock_server_url),
        };

        let client = JiraClient::new(&config).unwrap();
        let issues = client.search_issues("updated >= \"-6h\"").await;

        assert!(issues.is_ok());
        let list = issues.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].key, "CAN-101");
        assert_eq!(list[0].fields.summary, "Fix broken pipeline on focal");
        assert_eq!(list[0].fields.status.name, "In Progress");
    }

    #[tokio::test]
    async fn test_real_jira_connection() {
        if let Ok(config) = jira_news::config::Config::load() {
            println!("Testing real JIRA connection to: {}", config.jira.base_url);
            let client = JiraClient::new(&config.jira).unwrap();
            let issues = client.search_issues("updated >= \"-6h\"").await;

            match issues {
                Ok(list) => {
                    println!("Successfully connected and retrieved {} issues from real JIRA!", list.len());
                }
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("401") || err_str.contains("Unauthorized") || err_str.contains("403") {
                        println!("Successfully reached JIRA but authentication failed (expected if token is not fully valid yet): {}", err_str);
                    } else {
                        panic!("Real JIRA connection failed with unexpected error: {:?}", e);
                    }
                }
            }
        } else {
            println!("Skipping real JIRA connection test: config not found");
        }
    }

    #[tokio::test]
    async fn test_real_gemini_connection() {
        if let Ok(config) = jira_news::config::Config::load() {
            println!("Testing real Gemini connection");
            let result = gemini::generate_content(
                &config.gemini,
                "You are a test runner. Respond with exactly the word: SUCCESS",
                "Hello, please respond now."
            ).await;

            assert!(result.is_ok(), "Real Gemini API call failed: {:?}", result.err());
            let summary = result.unwrap();
            println!("Real Gemini response: {}", summary);
            assert!(summary.to_uppercase().contains("SUCCESS"));
        } else {
            println!("Skipping real Gemini API connection test: config not found");
        }
    }
}
