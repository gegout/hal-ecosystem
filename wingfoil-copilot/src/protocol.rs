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

#[derive(Debug, Deserialize, Clone)]
pub struct ApplicationRequest {
    pub request_id: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ProgressUpdate {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub request_id: String,
    pub percent: u32,
    pub message: String,
    pub format: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct FinalResponse {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub request_id: String,
    pub format: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trusted_html: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ErrorResponse {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub request_id: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub technical_details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_action: Option<String>,
    pub format: String,
}

pub fn send_progress(request_id: &str, percent: u32, message: &str) {
    let update = ProgressUpdate {
        msg_type: "progress".to_string(),
        request_id: request_id.to_string(),
        percent,
        message: message.to_string(),
        format: "html".to_string(),
    };
    if let Ok(serialized) = serde_json::to_string(&update) {
        println!("{}", serialized);
    }
}

pub fn send_final(request_id: &str, message: String) {
    let final_resp = FinalResponse {
        msg_type: "final".to_string(),
        request_id: request_id.to_string(),
        format: "html".to_string(),
        message,
        trusted_html: Some(true),
    };
    if let Ok(serialized) = serde_json::to_string(&final_resp) {
        println!("{}", serialized);
    }
}

pub fn send_error(
    request_id: &str,
    reason: String,
    technical_details: Option<String>,
    suggested_action: Option<String>,
) {
    let err_resp = ErrorResponse {
        msg_type: "error".to_string(),
        request_id: request_id.to_string(),
        reason,
        technical_details,
        suggested_action,
        format: "html".to_string(),
    };
    if let Ok(serialized) = serde_json::to_string(&err_resp) {
        println!("{}", serialized);
    }
}

/// Helper function to escape dynamic content for HTML to avoid breaking Telegram parsers.
pub fn escape_html(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '&' => escaped.push_str("&amp;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// Pre-sanitizes common HTML tags (like paragraph, break, lists) into clean Telegram-compatible formats.
pub fn pre_sanitize_html(html: &str) -> String {
    html.replace("<li>", "• ")
        .replace("</li>", "\n")
        .replace("<ul>", "")
        .replace("</ul>", "")
        .replace("<ol>", "")
        .replace("</ol>", "")
        .replace("<p>", "")
        .replace("</p>", "\n\n")
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<BR>", "\n")
        .replace("<BR/>", "\n")
        .replace("<BR />", "\n")
}

/// Validates that Telegram HTML tags in the message are properly opened and closed,
/// and that no unsupported tags are used.
pub fn is_html_balanced(html: &str) -> bool {
    let mut stack = Vec::new();
    let re = regex::Regex::new(r"</?([a-zA-Z][a-zA-Z0-9]*)[^>]*>").unwrap();
    // Full set of tags supported by Telegram's HTML parse mode.
    let allowed_tags = ["b", "strong", "i", "em", "u", "s", "del", "strike", "code", "pre", "a", "span", "tg-spoiler", "br", "hr", "img"];

    for cap in re.captures_iter(html) {
        let tag_full = cap.get(0).unwrap().as_str();
        let tag_name = cap.get(1).unwrap().as_str().to_lowercase();

        // Check if the tag is allowed by Telegram
        if !allowed_tags.contains(&tag_name.as_str()) {
            return false;
        }

        // Check if self-closing or standard
        let is_self_closing = tag_full.ends_with("/>") || tag_name == "br" || tag_name == "hr" || tag_name == "img";

        if tag_full.starts_with("</") {
            if let Some(open_tag) = stack.pop() {
                if open_tag != tag_name {
                    return false;
                }
            } else {
                return false;
            }
        } else if !is_self_closing {
            // Push to stack if not self-closing
            stack.push(tag_name);
        }
    }
    stack.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("Hello <World> & others"), "Hello &lt;World&gt; &amp; others");
    }

    #[test]
    fn test_is_html_balanced_valid() {
        assert!(is_html_balanced("<b>bold</b> and <i>italic</i>"));
        assert!(is_html_balanced("<code>some code</code>"));
        assert!(is_html_balanced("<pre>multiline\ncode</pre>"));
        assert!(is_html_balanced("<a href=\"http://example.com\">link</a>"));
        assert!(is_html_balanced("line break <br> and horizontal line <hr />"));
        assert!(is_html_balanced("wind speed is <20 km/h and wave height is <0.4 m"));
    }

    #[test]
    fn test_is_html_balanced_invalid_tag() {
        assert!(!is_html_balanced("<div>hello</div>"));
        assert!(!is_html_balanced("<script>alert(1)</script>"));
    }

    #[test]
    fn test_is_html_balanced_unbalanced() {
        assert!(!is_html_balanced("<b>bold without close"));
        assert!(!is_html_balanced("<b>bold <i>nested unbalanced</b></i>"));
        assert!(!is_html_balanced("</closing_only>"));
    }

    #[test]
    fn test_pre_sanitize_html() {
        let input = "<p>Paragraph</p><ul><li>Item 1</li><li>Item 2</li></ul><br>Break";
        let expected = "Paragraph\n\n• Item 1\n• Item 2\n\nBreak";
        assert_eq!(pre_sanitize_html(input), expected);
    }
}
