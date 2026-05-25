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
use crate::tool_registry::ToolRegistry;
use crate::registry::ApplicationRegistry;
use tracing::info;

pub struct ToolManager {
    tool_registry: Arc<ToolRegistry>,
    app_registry: Arc<ApplicationRegistry>,
}

impl ToolManager {
    pub fn new(tool_registry: Arc<ToolRegistry>, app_registry: Arc<ApplicationRegistry>) -> Self {
        Self {
            tool_registry,
            app_registry,
        }
    }

    pub async fn sync_capabilities(&self) {
        info!("Synchronizing HAL tool capabilities...");
        let registered_cmds = self.app_registry.get_all_commands().await;
        self.tool_registry.update_tools(registered_cmds).await;
        info!("Capabilities synchronized successfully.");
    }
}
