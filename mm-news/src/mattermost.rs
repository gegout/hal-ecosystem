// Copyright (c) 2026 Cedric Gegout
// Licensed under the MIT License

use anyhow::{anyhow, Result};
use reqwest::Client;
use std::time::Duration;

use crate::config::MattermostConfig;
use crate::models::{Channel, PostList, User};

pub struct MattermostClient {
    client: Client,
    base_url: String,
    token: String,
}

impl MattermostClient {
    pub fn new(config: &MattermostConfig) -> Result<Self> {
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
            token: config.personal_token.clone(),
        })
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.token)
    }

    pub async fn get_my_channels(&self) -> Result<Vec<Channel>> {
        let url = format!("{}/api/v4/users/me/channels", self.base_url);
        let res = self.client.get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(anyhow!("Failed to get my channels: {}", res.status()));
        }

        let channels = res.json::<Vec<Channel>>().await?;
        Ok(channels)
    }

    pub async fn get_channel_posts(&self, channel_id: &str, since: i64) -> Result<PostList> {
        let url = format!("{}/api/v4/channels/{}/posts", self.base_url, channel_id);
        let res = self.client.get(&url)
            .header("Authorization", self.auth_header())
            .query(&[("since", since.to_string())])
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(anyhow!("Failed to get posts for channel {}: {}", channel_id, res.status()));
        }

        let post_list = res.json::<PostList>().await?;
        Ok(post_list)
    }

    pub async fn get_users_by_ids(&self, user_ids: &[String]) -> Result<Vec<User>> {
        if user_ids.is_empty() {
            return Ok(vec![]);
        }

        let url = format!("{}/api/v4/users/ids", self.base_url);
        let res = self.client.post(&url)
            .header("Authorization", self.auth_header())
            .json(user_ids)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(anyhow!("Failed to resolve users: {}", res.status()));
        }

        let users = res.json::<Vec<User>>().await?;
        Ok(users)
    }
}
