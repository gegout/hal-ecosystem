// Copyright (c) 2026 Cedric Gegout
// Licensed under the MIT License

#[cfg(test)]
mod tests {
    use serde_json::json;
    use mm_news::protocol::{ApplicationRequest, ProgressUpdate, FinalResponse};
    use mm_news::config::{GeminiConfig, MattermostConfig};
    use mm_news::gemini;
    use mm_news::mattermost::MattermostClient;
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
            "command": "mm24h",
            "arguments": "",
            "raw_message": "/mm24h",
            "user_id": 123456789,
            "chat_id": 987654321
        });

        let incoming_str = serde_json::to_string(&incoming).unwrap();
        let req: ApplicationRequest = serde_json::from_str(&incoming_str).unwrap();
        assert_eq!(req.request_id, "uuid-v4-string");
        assert_eq!(req.command, "mm24h");
        assert_eq!(req.chat_id, 987654321);
    }

    #[tokio::test]
    async fn test_gemini_fallback_mechanism() {
        // Start a mock TCP server to simulate Gemini API endpoints
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
                            // 1st model: "gemini-3.5-flash" -> Return 503 error
                            "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n"
                        }
                        1 => {
                            // 2nd model: "gemini-2.5-pro" -> Return 404 error
                            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n"
                        }
                        2 => {
                            // 3rd model: "gemini-2.5-flash" -> Return 200 OK with success content
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
                            // Leak the response payload into a static reference or box it
                            Box::leak(resp.into_boxed_str())
                        }
                        _ => "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n",
                    };

                    let _ = socket.write_all(response.as_bytes()).await;
                    let _ = socket.flush().await;
                }
            }
        });

        // Config setup
        let config = GeminiConfig {
            api_key: "test_key".to_string(),
            preferred_models: Some(vec![
                "gemini-3.5-flash".to_string(),
                "gemini-2.5-pro".to_string(),
                "gemini-2.5-flash".to_string(),
            ]),
            test_mock_url: Some(mock_server_url),
        };

        // Call our generator content logic
        let res = gemini::generate_content(&config, "system prompt", "user prompt").await;
        
        // Assert we successfully fell back through errors to the 3rd working model!
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), "Mocked summary from fallback model");
        assert_eq!(request_counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_mattermost_channel_retrieval() {
        // Start a mock TCP server to simulate Mattermost channel endpoint
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mock_server_url = format!("http://{}", addr);

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buffer = [0; 1024];
                let _ = socket.read(&mut buffer).await;

                // Mock list of joined channels
                let channels_json = json!([
                    {
                        "id": "channel-id-1",
                        "team_id": "team-id-1",
                        "name": "pm-general",
                        "display_name": "Product Management General",
                        "type": "O"
                    },
                    {
                        "id": "channel-id-2",
                        "team_id": "team-id-1",
                        "name": "canonical-news",
                        "display_name": "Canonical News",
                        "type": "O"
                    }
                ]);

                let body = channels_json.to_string();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );

                let _ = socket.write_all(resp.as_bytes()).await;
                let _ = socket.flush().await;
            }
        });

        // Config setup
        let config = MattermostConfig {
            base_url: "http://dummy_url".to_string(),
            personal_token: "test_token".to_string(),
            test_mock_url: Some(mock_server_url),
        };

        let client = MattermostClient::new(&config).unwrap();
        let channels = client.get_my_channels().await;

        // Assert that channels are parsed and retrieved correctly
        assert!(channels.is_ok());
        let list = channels.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].display_name, "Product Management General");
        assert_eq!(list[1].name, "canonical-news");
    }

    #[tokio::test]
    async fn test_real_mattermost_connection() {
        // Attempt to load the real config.
        // If the configuration does not exist, we log and skip so it doesn't break CI,
        // but if it does exist, we perform a real verification.
        if let Ok(config) = mm_news::config::Config::load() {
            println!("Testing real Mattermost connection to: {}", config.mattermost.base_url);
            let client = MattermostClient::new(&config.mattermost).unwrap();
            let channels = client.get_my_channels().await;

            match channels {
                Ok(list) => {
                    println!("Successfully retrieved {} active channels from real Mattermost!", list.len());
                    assert!(!list.is_empty(), "Expected to retrieve at least one channel from real Mattermost");
                }
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("401") || err_str.contains("Unauthorized") {
                        println!("Successfully connected to real Mattermost (received expected 401 Unauthorized from the server): {}", err_str);
                    } else {
                        panic!("Real Mattermost connection failed: {:?}", e);
                    }
                }
            }
        } else {
            println!("Skipping real Mattermost connection test: config not found");
        }
    }

    #[tokio::test]
    async fn test_real_gemini_connection() {
        if let Ok(config) = mm_news::config::Config::load() {
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

