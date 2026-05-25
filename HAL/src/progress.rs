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

pub fn escape_html(input: &str) -> String {
    let mut escaped = String::new();
    for c in input.chars() {
        match c {
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#x27;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

pub fn format_progress_message(percent: u32, message: &str) -> String {
    let percent = percent.min(100);
    let filled_blocks = (percent / 10) as usize;
    let empty_blocks = 10 - filled_blocks;

    let mut bar = String::new();
    for _ in 0..filled_blocks {
        bar.push('█');
    }
    for _ in 0..empty_blocks {
        bar.push('░');
    }

    let escaped_msg = escape_html(message);

    format!(
        "🔴 <b>[HAL]</b>\n\n\
        <b>Status:</b> processing request...\n\
        <pre>[{}] {}%</pre>\n\n\
        <b>Telemetry:</b> <i>{}</i>",
        bar, percent, escaped_msg
    )
}

pub fn format_error_card(reason: &str, details: Option<&str>, action: Option<&str>) -> String {
    let escaped_reason = escape_html(reason);
    let mut card = format!(
        "🔴 <b>[HAL: Operational Exception]</b>\n\n\
        <i>\"I'm afraid I can't do that.\"</i>\n\n\
        <b>Diagnostic:</b> <i>{}</i>\n",
        escaped_reason
    );

    if let Some(det) = details {
        card.push_str(&format!(
            "<b>Details:</b>\n<pre>{}</pre>\n",
            escape_html(det)
        ));
    }

    if let Some(act) = action {
        card.push_str(&format!(
            "<b>Suggested Action:</b> {}\n",
            escape_html(act)
        ));
    }

    card
}
