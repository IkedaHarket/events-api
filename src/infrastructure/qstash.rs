use crate::domain::services::scheduler::{NotificationScheduler, WebhookEventPayload};
use async_trait::async_trait;
use chrono::Utc;
use ikeda_core::error::AppError;
use reqwest::Client;
use serde::Deserialize;
use std::sync::Mutex;
use uuid::Uuid;

/// Configuration options for the Upstash QStash client.
#[derive(Debug, Clone)]
pub struct QStashConfig {
    /// QStash publish endpoint base URL (defaults to "https://qstash.upstash.io/v2/publish").
    pub base_url: String,
    /// Upstash QStash Bearer API Token.
    pub token: String,
}

impl Default for QStashConfig {
    fn default() -> Self {
        Self {
            base_url: "https://qstash.upstash.io/v2/publish".to_string(),
            token: String::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct QStashResponse {
    #[serde(rename = "messageId")]
    message_id: String,
}

/// Upstash QStash adapter implementing `NotificationScheduler`.
pub struct QStashScheduler {
    client: Client,
    config: QStashConfig,
}

impl QStashScheduler {
    /// Creates a new QStashScheduler instance.
    pub fn new(config: QStashConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }
}

#[async_trait]
impl NotificationScheduler for QStashScheduler {
    async fn schedule_reminder(
        &self,
        tenant_id: Uuid,
        destination_url: &str,
        scheduled_for: chrono::DateTime<Utc>,
        payload: WebhookEventPayload,
    ) -> Result<String, AppError> {
        if self.config.token.is_empty() {
            return Err(AppError::InternalServerError(
                "Upstash QStash token is not configured".to_string(),
            ));
        }

        let delay_seconds = (scheduled_for - Utc::now()).num_seconds().max(0);
        let publish_url = format!(
            "{}/{}",
            self.config.base_url.trim_end_matches('/'),
            destination_url
        );

        let response = self
            .client
            .post(&publish_url)
            .bearer_auth(&self.config.token)
            .header("Upstash-Delay", format!("{delay_seconds}s"))
            .header("Upstash-Retries", "3")
            .header("X-Tenant-Id", tenant_id.to_string())
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to dispatch to QStash: {e}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::InternalServerError(format!(
                "QStash API rejected message (status {status}): {body}"
            )));
        }

        let qstash_res: QStashResponse = response.json().await.map_err(|e| {
            AppError::InternalServerError(format!("Failed to parse QStash response: {e}"))
        })?;

        Ok(qstash_res.message_id)
    }
}

/// In-memory scheduler for automated tests and development mode.
#[derive(Default)]
pub struct MockNotificationScheduler {
    pub scheduled_reminders: Mutex<Vec<(String, chrono::DateTime<Utc>, WebhookEventPayload)>>,
}

#[async_trait]
impl NotificationScheduler for MockNotificationScheduler {
    async fn schedule_reminder(
        &self,
        _tenant_id: Uuid,
        destination_url: &str,
        scheduled_for: chrono::DateTime<Utc>,
        payload: WebhookEventPayload,
    ) -> Result<String, AppError> {
        let msg_id = format!("mock_msg_{}", Uuid::new_v4());
        self.scheduled_reminders.lock().unwrap().push((
            destination_url.to_string(),
            scheduled_for,
            payload,
        ));
        Ok(msg_id)
    }
}
