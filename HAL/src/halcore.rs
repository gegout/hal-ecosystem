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

use std::time::Duration;
use tracing::info;

use crate::config::Config;
use crate::protocol::{ApplicationRequest, ApplicationResponse, FinalResponse};
use crate::registry::ApplicationDefinition;
use crate::transport::{ApplicationTransport, HttpTransport, StdioTransport};

pub struct HalCoreBridge {
    config: Config,
}

impl HalCoreBridge {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn call(
        &self,
        request: ApplicationRequest,
        progress_sink: tokio::sync::mpsc::Sender<ApplicationResponse>,
    ) -> Result<FinalResponse, anyhow::Error> {
        info!("Forwarding request {} to HALcore", request.request_id);

        let app_def = ApplicationDefinition {
            name: "HALcore".to_string(),
            description: Some("HAL Fallback and chatbot".to_string()),
            transport: self.config.halcore.transport.clone(),
            command: self.config.halcore.command.clone(),
            url: self.config.halcore.url.clone(),
            commands: vec![],
        };

        let timeout_dur = Duration::from_secs(self.config.halcore.timeout_seconds.unwrap_or(60));
        let transport: Box<dyn ApplicationTransport> = if app_def.transport == "stdio" {
            let cmd_path = app_def.command.as_deref().unwrap_or("").into();
            Box::new(StdioTransport {
                command_path: cmd_path,
                timeout_duration: timeout_dur,
            })
        } else {
            let url = app_def.url.as_deref().unwrap_or("").to_string();
            Box::new(HttpTransport {
                url,
                timeout_duration: timeout_dur,
            })
        };

        transport.call(request, progress_sink).await
    }
}
