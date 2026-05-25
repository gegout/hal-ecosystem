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

use tracing::info;

use crate::models::{CorrectedForecast, ForecastHour, HolfuyObservation};

pub fn compute_corrections(
    holfuy: &HolfuyObservation,
    forecasts: &[ForecastHour],
    wind_weight: f64,
    gust_weight: f64,
) -> Vec<CorrectedForecast> {
    info!("Computing forecast correction (weights: wind={}, gust={})", wind_weight, gust_weight);
    
    // In a real scenario, we match current time with current forecast hour
    let default_forecast = ForecastHour {
        hour: "now".to_string(),
        wind_kmh: 0.0,
        gust_kmh: 0.0,
        wave_m: None,
        direction: None,
    };
    let current_forecast = forecasts.first().unwrap_or(&default_forecast);
    
    let holfuy_avg_kmh = holfuy.avg15_knots.unwrap_or(0.0) * 1.852;
    let holfuy_gust_kmh = holfuy.hour_max_gust_knots.unwrap_or(holfuy_avg_kmh * 1.2) * 1.852;
 
    let delta_wind = holfuy_avg_kmh - current_forecast.wind_kmh;
    let delta_gust = holfuy_gust_kmh - current_forecast.gust_kmh;
 
    forecasts.iter().map(|f| {
        let corrected_wind = f.wind_kmh + wind_weight * delta_wind;
        let corrected_gust = f.gust_kmh + gust_weight * delta_gust;
        
        CorrectedForecast {
            hour: f.hour.clone(),
            corrected_wind,
            corrected_gust,
            wave_m: f.wave_m,
            wingfoil_ok: false, // will be evaluated later
        }
    }).collect()
}
