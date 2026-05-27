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

use axum::http::HeaderMap;
use crate::config::HttpConfig;

/// Check if the request is authorized based on the HttpConfig.
///
/// Rules:
/// - If `api_keys` is empty:
///   - Authentication disabled
///   - Local access allowed
/// - If `api_keys` contains entries:
///   - Authentication required
///
/// Authentication format:
/// ```http
/// Authorization: Bearer <api_key>
/// ```
///
/// Valid if:
/// - Token exactly matches one configured key (after trimming whitespace and ignoring empty keys)
pub fn is_authorized(
    headers: &HeaderMap,
    config: &HttpConfig
) -> bool {
    let keys = config.normalized_api_keys();

    if keys.is_empty() {
        return true;
    }

    let Some(value) = headers.get("authorization") else {
        return false;
    };

    let Ok(value) = value.to_str() else {
        return false;
    };

    let Some(token) = value.strip_prefix("Bearer ") else {
        return false;
    };

    keys.iter().any(|x| x == token.trim())
}
