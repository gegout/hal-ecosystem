// Copyright (c) 2026 Cedric Gegout
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
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

use anyhow::{Context, Result};
use chrono::Utc;
use headless_chrome::{Browser, LaunchOptions};
use regex::Regex;
use std::time::Duration;
use tracing::{info, warn};

use crate::config::Config;
use crate::models::HolfuyObservation;
use crate::ocr;

pub async fn collect_holfuy_data(config: &Config) -> Result<HolfuyObservation> {
    info!("Opening Holfuy page");

    let launch_options = LaunchOptions {
        headless: config.browser.headless,
        ..Default::default()
    };
    
    info!("Launching Chromium headless");
    let browser = Browser::new(launch_options)
        .context("Failed to launch headless chrome")?;

    let tab = browser.new_tab()
        .context("Failed to create new browser tab")?;
        
    info!("Navigating to {}", config.holfuy.url);
    tab.navigate_to(&config.holfuy.url)
        .context("Failed to navigate to Holfuy URL")?;
        
    info!("Waiting for dynamic rendering");
    std::thread::sleep(Duration::from_millis(config.browser.wait_after_load_ms));
    tab.wait_until_navigated()
        .context("Failed waiting for page navigation")?;
        
    info!("Extracting rendered DOM");
    let html = tab.get_content()
        .context("Failed to get HTML content from tab")?;

    // Strategy 1: Intercept or extract directly from DOM
    let observation = parse_holfuy_html(&html);

    // Strategy 2: Fallback to OCR if incomplete and enabled
    if (observation.avg15_knots.is_none() || observation.hour_max_gust_knots.is_none()) && config.browser.ocr_enabled {
        warn!("DOM extraction incomplete, falling back to OCR");
        let screenshot_dir = dirs::cache_dir()
            .unwrap_or_default()
            .join("wingfoil-copilot")
            .join("screenshots");
            
        std::fs::create_dir_all(&screenshot_dir).ok();
        let screenshot_path = screenshot_dir.join("holfuy_capture.png");
        
        let png_data = tab.capture_screenshot(
            headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png,
            None,
            None,
            true
        ).context("Failed to capture screenshot")?;
        
        std::fs::write(&screenshot_path, png_data)
            .context("Failed to save screenshot")?;
            
        let ocr_text = ocr::extract_text_from_image(screenshot_path.to_str().unwrap())?;
        info!("OCR Result length: {}", ocr_text.len());
        
        // OCR enhancement logic can be implemented here if needed.
        // enhance_observation_with_ocr(&mut observation, &ocr_text);
    }
    
    info!("Parsing measurements completed");
    Ok(observation)
}

fn parse_holfuy_html(html: &str) -> HolfuyObservation {
    // Attempt basic extraction - in a real scenario we'd use scraper on precise IDs
    // We use a regex heuristic for demonstration of the extraction structure
    let wind_re = Regex::new(r"(?i)(\d+(\.\d+)?)\s*(knots|kts|km/h)").unwrap();
    
    let mut avg15 = None;
    if let Some(caps) = wind_re.captures(html) {
        if let Ok(val) = caps[1].parse::<f64>() {
            avg15 = Some(val);
        }
    }
    
    HolfuyObservation {
        instant_knots: avg15, // Approximation
        avg15_knots: avg15,
        hour_avg_knots: avg15,
        hour_max_gust_knots: avg15.map(|v| v * 1.2), // Rough heuristic if missing
        direction: Some("NW".to_string()), // Placeholder
        timestamp: Utc::now().to_rfc3339(),
    }
}


