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

use crate::config::WingfoilConfig;
use crate::models::CorrectedForecast;

pub fn evaluate_rules(forecasts: &mut Vec<CorrectedForecast>, rules: &WingfoilConfig) {
    for f in forecasts.iter_mut() {
        let wind_ok = f.corrected_wind >= rules.min_average_wind_kmh;
        let gust_ok = f.corrected_gust <= rules.max_gust_kmh;
        let wave_ok = f.wave_m.unwrap_or(0.0) <= rules.max_wave_height_m;
        
        f.wingfoil_ok = wind_ok && gust_ok && wave_ok;
    }
}
