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
use headless_chrome::{Browser, LaunchOptions};
use scraper::{Html, Selector};
use std::time::Duration;
use tracing::info;

use crate::config::Config;
use crate::models::ForecastHour;

pub async fn collect_forecasts(config: &Config) -> Result<Vec<ForecastHour>> {
    info!("Opening MeteoConsult page via headless browser");

    let launch_options = LaunchOptions {
        headless: config.browser.headless,
        args: vec![
            std::ffi::OsStr::new("--no-sandbox"),
            std::ffi::OsStr::new("--disable-setuid-sandbox"),
            // Spoof a real browser UA. The default headless Chrome UA causes
            // MeteoConsult's bot-detection to return a ~390-byte rejection page.
            std::ffi::OsStr::new("--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36"),
            // Disable the 'webdriver' flag that sites use to detect automation.
            std::ffi::OsStr::new("--disable-blink-features=AutomationControlled"),
        ],
        ..Default::default()
    };

    info!("Launching Chromium headless for MeteoConsult");
    let browser = Browser::new(launch_options)
        .context("Failed to launch headless chrome for MeteoConsult")?;

    let tab = browser.new_tab()
        .context("Failed to create new browser tab")?;

    info!("Navigating to {}", config.meteoconsult.url);
    tab.navigate_to(&config.meteoconsult.url)
        .context("Failed to navigate to MeteoConsult URL")?;

    info!("Waiting for initial navigation");
    tab.wait_until_navigated()
        .context("Failed waiting for MeteoConsult page navigation")?;

    info!("Waiting for MeteoConsult JS rendering ({} ms)", config.browser.wait_after_load_ms);
    std::thread::sleep(Duration::from_millis(config.browser.wait_after_load_ms));

    info!("Extracting rendered MeteoConsult DOM");
    let html = tab.get_content()
        .context("Failed to get MeteoConsult HTML content")?;

    info!("Parsing MeteoConsult forecast data (HTML length: {} bytes)", html.len());
    parse_meteoconsult_html(&html)
}

/// Parse the MeteoConsult page HTML (supports both old comparator page and new Spot page) and extract hourly forecasts.
///
/// # Selectors & Elements parsed:
/// - Hours: `ul.hours` or `ul.th.hours`
/// - Wind Speed: `ul.wrc-wind_speed` or `ul.wind-speed` (picks `span.multi-speed-kmh span.text`)
/// - Wind Gust: `ul.wrc-wind_gust` or `ul.wind-gust` (picks `span.multi-speed-kmh span.text`)
/// - Wind Direction: `ul.wrc-wind_direction` or `ul.wind-direction.value` (picks cardinal16, cardinal8, or degree)
/// - Wave Height: `ul.wrc-wave_height` or `ul.wave-height`
fn parse_meteoconsult_html(html: &str) -> Result<Vec<ForecastHour>> {
    let document = Html::parse_document(html);

    let ul_hours = document.select(&Selector::parse("ul.hours").unwrap())
        .next()
        .or_else(|| document.select(&Selector::parse("ul.th.hours").unwrap()).next());
        
    let ul_wind = document.select(&Selector::parse("ul.wrc-wind_speed").unwrap())
        .next()
        .or_else(|| document.select(&Selector::parse("ul.wind-speed").unwrap()).next());

    let ul_gust = document.select(&Selector::parse("ul.wrc-wind_gust").unwrap())
        .next()
        .or_else(|| document.select(&Selector::parse("ul.wind-gust").unwrap()).next());

    let ul_dir = document.select(&Selector::parse("ul.wrc-wind_direction").unwrap())
        .next()
        .or_else(|| document.select(&Selector::parse("ul.wind-direction.value").unwrap()).next());

    let ul_wave = document.select(&Selector::parse("ul.wrc-wave_height").unwrap())
        .next()
        .or_else(|| document.select(&Selector::parse("ul.wave-height").unwrap()).next());

    let li_sel = Selector::parse("li").unwrap();
    let span_sel = Selector::parse("span").unwrap();
    let kmh_sel = Selector::parse("span.multi-speed-kmh span.text").unwrap();

    let hours: Vec<String> = ul_hours
        .map(|ul| {
            ul.select(&li_sel)
                .map(|li| {
                    li.select(&span_sel)
                        .next()
                        .map(|s| s.text().collect::<String>().trim().to_string())
                        .unwrap_or_else(|| li.text().collect::<String>().trim().to_string())
                })
                .collect()
        })
        .unwrap_or_default();

    let winds: Vec<String> = ul_wind
        .map(|ul| {
            ul.select(&li_sel)
                .map(|li| {
                    li.select(&kmh_sel)
                        .next()
                        .map(|s| s.text().collect::<String>().trim().to_string())
                        .unwrap_or_else(|| li.text().collect::<String>().trim().to_string())
                })
                .collect()
        })
        .unwrap_or_default();

    let gusts: Vec<String> = ul_gust
        .map(|ul| {
            ul.select(&li_sel)
                .map(|li| {
                    li.select(&kmh_sel)
                        .next()
                        .map(|s| s.text().collect::<String>().trim().to_string())
                        .unwrap_or_else(|| li.text().collect::<String>().trim().to_string())
                })
                .collect()
        })
        .unwrap_or_default();

    let dirs: Vec<String> = ul_dir
        .map(|ul| {
            ul.select(&li_sel)
                .map(|li| {
                    let card16_sel = Selector::parse("span.multi-cardinal-cardinal16 span").unwrap();
                    let card8_sel = Selector::parse("span.multi-cardinal-cardinal8 span").unwrap();
                    let degree_sel = Selector::parse("span.multi-cardinal-degree span").unwrap();
                    
                    if let Some(e) = li.select(&card16_sel).next() {
                        e.text().collect::<String>().trim().to_string()
                    } else if let Some(e) = li.select(&card8_sel).next() {
                        e.text().collect::<String>().trim().to_string()
                    } else if let Some(e) = li.select(&degree_sel).next() {
                        let deg = e.text().collect::<String>().trim().to_string();
                        if deg.is_empty() {
                            "".to_string()
                        } else {
                            format!("{}°", deg)
                        }
                    } else {
                        li.text().collect::<String>().trim().to_string()
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let waves: Vec<String> = ul_wave
        .map(|ul| {
            ul.select(&li_sel)
                .map(|li| li.text().collect::<String>().trim().to_string())
                .collect()
        })
        .unwrap_or_default();

    info!(
        "MeteoConsult extracted: {} hours, {} wind values, {} gusts, {} directions, {} waves",
        hours.len(), winds.len(), gusts.len(), dirs.len(), waves.len()
    );

    if hours.is_empty() || winds.is_empty() {
        return Err(anyhow::anyhow!(
            "MeteoConsult parsing yielded no forecast data. \
             The page structure may have changed. \
             HTML length was {} bytes.",
            html.len()
        ));
    }

    let count = hours.len()
        .min(winds.len())
        .min(gusts.len())
        .min(dirs.len());

    let forecasts = (0..count)
        .map(|i| {
            let wave_val = waves.get(i)
                .and_then(|w| w.parse::<f64>().ok());
            ForecastHour {
                hour: hours[i].clone(),
                wind_kmh: winds[i].parse::<f64>().unwrap_or(0.0),
                gust_kmh: gusts[i].parse::<f64>().unwrap_or(0.0),
                wave_m: wave_val,
                direction: if dirs[i].is_empty() { None } else { Some(dirs[i].clone()) },
            }
        })
        .collect();

    Ok(forecasts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_known_html_snippet() {
        // Minimal representative fragment of the MeteoConsult comparator page.
        let html = r#"
        <html><body>
        <ul class="th hours">
            <li><span>12h</span></li>
            <li><span>13h</span></li>
        </ul>
        <ul class="wind-speed">
            <li class="multi-speed WindLevel5">
                <span class="multi-speed-kmh show"><span class="text">25</span></span>
            </li>
            <li class="multi-speed WindLevel4">
                <span class="multi-speed-kmh show"><span class="text">22</span></span>
            </li>
        </ul>
        <ul class="wind-gust">
            <li class="multi-speed WindLevel6">
                <span class="multi-speed-kmh show"><span class="text">38</span></span>
            </li>
            <li class="multi-speed WindLevel5">
                <span class="multi-speed-kmh show"><span class="text">33</span></span>
            </li>
        </ul>
        <ul class="wind-direction value">
            <li class="multi-cardinal">
                <span class="multi-cardinal-cardinal16"><span>NW</span></span>
            </li>
            <li class="multi-cardinal">
                <span class="multi-cardinal-cardinal16"><span>WNW</span></span>
            </li>
        </ul>
        </body></html>
        "#;

        let result = parse_meteoconsult_html(html).expect("parsing should succeed");
        assert_eq!(result.len(), 2);

        assert_eq!(result[0].hour, "12h");
        assert_eq!(result[0].wind_kmh, 25.0);
        assert_eq!(result[0].gust_kmh, 38.0);
        assert_eq!(result[0].direction, Some("NW".to_string()));

        assert_eq!(result[1].hour, "13h");
        assert_eq!(result[1].wind_kmh, 22.0);
        assert_eq!(result[1].gust_kmh, 33.0);
        assert_eq!(result[1].direction, Some("WNW".to_string()));
    }

    #[test]
    fn test_parse_returns_error_on_empty_html() {
        let result = parse_meteoconsult_html("<html><body></body></html>");
        assert!(result.is_err(), "empty page should return an error");
    }
}
