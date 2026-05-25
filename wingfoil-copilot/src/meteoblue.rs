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
use serde::Deserialize;
use tracing::info;

use crate::config::Config;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MeteoblueForecastHour {
    pub hour: String,
    pub wind_kmh: f64,
    pub gust_kmh: f64,
    pub wave_m: Option<f64>,
    pub direction: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MeteoblueResponse {
    data_1h: Option<MeteoblueData1h>,
}

#[derive(Debug, Deserialize)]
struct MeteoblueData1h {
    time: Vec<String>,
    // Wind Speed at 10m is typically named 'windspeed' in the Forecast API basic-1h package
    windspeed: Option<Vec<f64>>,
    // Wind Gust at surface is typically named 'windgust' in the wind-1h package
    windgust: Option<Vec<f64>>,
    // Wind Direction at 10m is typically named 'winddirection' in the basic-1h or wind-1h package
    winddirection: Option<Vec<f64>>,
    // Significant Wave Height is typically named 'significantwaveheight' in the sea-1h package
    significantwaveheight: Option<Vec<f64>>,
    // Secondary fallback naming options for robustness
    waveheight: Option<Vec<f64>>,
}

pub async fn collect_forecasts(config: &Config) -> Result<Vec<MeteoblueForecastHour>> {
    info!("Querying Meteoblue Forecast packages API");
    
    // Perform robust HTTP GET request
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .context("Failed to build HTTP client for Meteoblue")?;

    let response = client.get(&config.meteoblue.url)
        .send()
        .await
        .context("Failed to fetch forecast from Meteoblue API endpoint")?;

    if !response.status().is_success() {
        let status = response.status();
        let body_err = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "Meteoblue API returned error status: {}. Response: {}",
            status, body_err
        ));
    }

    let raw_json = response.text().await
        .context("Failed to read Meteoblue response text")?;
    
    info!("Meteoblue JSON response received ({} bytes)", raw_json.len());
    parse_meteoblue_json(&raw_json)
}

pub fn parse_meteoblue_json(json_str: &str) -> Result<Vec<MeteoblueForecastHour>> {
    let parsed: MeteoblueResponse = serde_json::from_str(json_str)
        .context("Failed to parse Meteoblue JSON schema")?;

    let data = parsed.data_1h.context("Meteoblue response is missing 'data_1h' block")?;
    
    let times = data.time;
    if times.is_empty() {
        return Ok(Vec::new());
    }

    let winds = data.windspeed.unwrap_or_default();
    let gusts = data.windgust.unwrap_or_default();
    let dirs = data.winddirection.unwrap_or_default();
    
    // Wave height robust fallback (try 'significantwaveheight' first, then 'waveheight')
    let waves = data.significantwaveheight.or(data.waveheight).unwrap_or_default();

    let count = times.len();
    info!(
        "Meteoblue parsed: {} hours, {} winds, {} gusts, {} directions, {} wave heights",
        count, winds.len(), gusts.len(), dirs.len(), waves.len()
    );

    let mut forecasts = Vec::with_capacity(count);
    for i in 0..count {
        let full_time = &times[i];
        // Meteoblue times can be formatted like "2026-05-25 14:00" or "2026-05-25T14:00:00"
        // Let's extract just the hour component (e.g. "14h") to be uniform with MeteoConsult
        let hour_str = extract_hour_label(full_time);

        let wind_val = winds.get(i).copied().unwrap_or(0.0);
        let gust_val = gusts.get(i).copied().unwrap_or(wind_val * 1.2);
        
        let dir_val = dirs.get(i).copied();
        let cardinal_dir = dir_val.map(degree_to_cardinal);

        let wave_val = waves.get(i).copied();

        forecasts.push(MeteoblueForecastHour {
            hour: hour_str,
            wind_kmh: wind_val,
            gust_kmh: gust_val,
            wave_m: wave_val,
            direction: cardinal_dir,
        });
    }

    Ok(forecasts)
}

fn extract_hour_label(time_str: &str) -> String {
    // E.g. "2026-05-25 14:00" -> find index of ' ' or 'T', then take 2 digits after it
    let delimiter = if time_str.contains('T') { 'T' } else { ' ' };
    if let Some(pos) = time_str.find(delimiter) {
        let hour_part = &time_str[pos + 1..];
        if hour_part.len() >= 2 {
            let hour_digits = &hour_part[..2];
            if let Ok(h) = hour_digits.parse::<u32>() {
                return format!("{:02}h", h);
            }
        }
    }
    // Fallback
    time_str.to_string()
}

pub fn degree_to_cardinal(deg: f64) -> String {
    let degrees = (deg % 360.0 + 360.0) % 360.0;
    let index = ((degrees + 11.25) / 22.5).floor() as usize;
    let cardinals = [
        "N", "NNE", "NE", "ENE",
        "E", "ESE", "SE", "SSE",
        "S", "SSW", "SW", "WSW",
        "W", "WNW", "NW", "NNW",
        "N",
    ];
    cardinals[index].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_meteoblue_example_json() {
        let json = r#"
        {
            "metadata": {},
            "data_1h": {
                "time": ["2026-05-25 08:00", "2026-05-25 09:00"],
                "windspeed": [15.5, 18.2],
                "windgust": [22.4, 25.1],
                "winddirection": [180.0, 225.0],
                "significantwaveheight": [0.4, 0.5]
            }
        }
        "#;

        let res = parse_meteoblue_json(json).expect("should parse successfully");
        assert_eq!(res.len(), 2);
        
        assert_eq!(res[0].hour, "08h");
        assert_eq!(res[0].wind_kmh, 15.5);
        assert_eq!(res[0].gust_kmh, 22.4);
        assert_eq!(res[0].direction, Some("S".to_string()));
        assert_eq!(res[0].wave_m, Some(0.4));

        assert_eq!(res[1].hour, "09h");
        assert_eq!(res[1].direction, Some("SW".to_string()));
        assert_eq!(res[1].wave_m, Some(0.5));
    }

    #[test]
    fn test_degree_to_cardinal() {
        assert_eq!(degree_to_cardinal(0.0), "N");
        assert_eq!(degree_to_cardinal(90.0), "E");
        assert_eq!(degree_to_cardinal(180.0), "S");
        assert_eq!(degree_to_cardinal(270.0), "W");
        assert_eq!(degree_to_cardinal(350.0), "N");
    }
}
