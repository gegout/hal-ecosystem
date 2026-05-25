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

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolfuyObservation {
    pub instant_knots: Option<f64>,
    pub avg15_knots: Option<f64>,
    pub hour_avg_knots: Option<f64>,
    pub hour_max_gust_knots: Option<f64>,
    pub direction: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForecastHour {
    pub hour: String,
    pub wind_kmh: f64,
    pub gust_kmh: f64,
    pub wave_m: Option<f64>,
    pub direction: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectedForecast {
    pub hour: String,
    pub corrected_wind: f64,
    pub corrected_gust: f64,
    pub wave_m: Option<f64>,
    pub wingfoil_ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombinedHourlyForecast {
    pub hour: String,
    pub meteoconsult_wind_kmh: f64,
    pub meteoconsult_gust_kmh: f64,
    pub meteoconsult_corrected_wind_kmh: f64,
    pub meteoconsult_corrected_gust_kmh: f64,
    pub meteoconsult_wave_m: Option<f64>,
    pub meteoconsult_direction: Option<String>,
    pub meteoblue_wind_kmh: Option<f64>,
    pub meteoblue_gust_kmh: Option<f64>,
    pub meteoblue_wave_m: Option<f64>,
    pub meteoblue_direction: Option<String>,
}
