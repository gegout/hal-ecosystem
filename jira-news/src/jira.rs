// Copyright (c) 2026 Cedric Gegout
// Licensed under the MIT License

use anyhow::{anyhow, Result};
use reqwest::Client;
use std::time::Duration;

use crate::config::JiraConfig;
use crate::models::{JiraIssue, JiraSearchResult};

pub struct JiraClient {
    client: Client,
    base_url: String,
    config: JiraConfig,
}

impl JiraClient {
    pub fn new(config: &JiraConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        let base_url = match &config.test_mock_url {
            Some(mock) => mock.trim_end_matches('/').to_string(),
            None => config.base_url.trim_end_matches('/').to_string(),
        };

        Ok(Self {
            client,
            base_url,
            config: config.clone(),
        })
    }

    fn add_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(email) = &self.config.user_email {
            builder.basic_auth(email, Some(&self.config.token))
        } else {
            builder.header("Authorization", format!("Bearer {}", self.config.token))
        }
    }

    pub async fn search_issues(&self, jql: &str) -> Result<Vec<JiraIssue>> {
        let url = format!("{}/rest/api/3/search/jql", self.base_url);
        let mut all_issues = Vec::new();
        let mut start_at = 0;
        let max_results = 100;

        loop {
            let mut req = self.client.get(&url)
                .query(&[
                    ("jql", jql),
                    ("fields", "key,summary,status,assignee,updated,description,comment"),
                    ("startAt", &start_at.to_string()),
                    ("maxResults", &max_results.to_string()),
                ]);

            req = self.add_auth(req);
            
            let res = req.send().await?;

            if !res.status().is_success() {
                let status = res.status();
                let body = res.text().await.unwrap_or_default();
                return Err(anyhow!("Failed to search Jira issues: {}. Response: {}", status, body));
            }

            let search_result = res.json::<JiraSearchResult>().await?;
            let issues_len = search_result.issues.len();
            
            all_issues.extend(search_result.issues);

            let total = search_result.total.unwrap_or(0);
            start_at += issues_len;

            if start_at >= total as usize || issues_len == 0 {
                break;
            }
        }

        Ok(all_issues)
    }
}
