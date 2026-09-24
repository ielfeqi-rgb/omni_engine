use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub struct WebLens {
    client: reqwest::blocking::Client,
}

impl WebLens {
    pub fn new() -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(12))
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        Self { client }
    }

    /// Primary search dispatcher with multi-source fallback:
    /// 1. DuckDuckGo HTML / Lite
    /// 2. Google News RSS
    /// 3. Wikipedia API
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, String> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Err("Empty search query".to_string());
        }

        // Try DuckDuckGo Lite first
        match self.search_duckduckgo(trimmed, limit) {
            Ok(results) if !results.is_empty() => return Ok(results),
            _ => {}
        }

        // Try Google RSS fallback
        match self.search_google_rss(trimmed, limit) {
            Ok(results) if !results.is_empty() => return Ok(results),
            _ => {}
        }

        // Try Wikipedia API fallback
        match self.search_wikipedia(trimmed, limit) {
            Ok(results) if !results.is_empty() => return Ok(results),
            _ => {}
        }

        Err(format!("No search results found for query: '{}'", trimmed))
    }

    /// Search DuckDuckGo Lite (lightweight, zero-javascript HTML)
    fn search_duckduckgo(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, String> {
        let url = format!("https://lite.duckduckgo.com/lite/");
        let params = [("q", query)];

        let response = self.client.post(&url)
            .form(&params)
            .send()
            .map_err(|e| format!("DuckDuckGo request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("DuckDuckGo HTTP status {}", response.status()));
        }

        let html = response.text().map_err(|e| format!("Failed to read DDG response: {}", e))?;
        let results = Self::parse_duckduckgo_lite(&html, limit);
        Ok(results)
    }

    /// Parse DuckDuckGo Lite HTML response into clean SearchResults
    pub fn parse_duckduckgo_lite(html: &str, limit: usize) -> Vec<SearchResult> {
        let mut results = Vec::new();
        let mut cursor = 0;

        while let Some(link_start) = html[cursor..].find("class=\"result-link\"") {
            if results.len() >= limit {
                break;
            }
            let pos = cursor + link_start;
            // Find href
            if let Some(href_pos) = html[pos..].find("href=\"") {
                let href_start = pos + href_pos + 6;
                if let Some(href_end) = html[href_start..].find('"') {
                    let mut raw_url = html[href_start..href_start + href_end].to_string();
                    // Un-redirect uddg if present
                    if let Some(uddg_idx) = raw_url.find("uddg=") {
                        let enc = &raw_url[uddg_idx + 5..];
                        let end_enc = enc.find('&').unwrap_or(enc.len());
                        if let Ok(decoded) = urlencoding::decode(&enc[..end_enc]) {
                            raw_url = decoded.to_string();
                        }
                    }

                    // Find link text (Title)
                    let title = if let Some(tag_end) = html[href_start + href_end..].find('>') {
                        let title_start = href_start + href_end + tag_end + 1;
                        if let Some(tag_close) = html[title_start..].find("</a>") {
                            Self::strip_html_tags(&html[title_start..title_start + tag_close])
                        } else {
                            "Untitled".to_string()
                        }
                    } else {
                        "Untitled".to_string()
                    };

                    // Find Snippet
                    let snippet = if let Some(snip_start) = html[pos..].find("class=\"result-snippet\"") {
                        let snip_pos = pos + snip_start;
                        if let Some(content_start) = html[snip_pos..].find('>') {
                            let text_start = snip_pos + content_start + 1;
                            if let Some(snip_end) = html[text_start..].find("</td>") {
                                Self::strip_html_tags(&html[text_start..text_start + snip_end])
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };

                    if !raw_url.is_empty() && !title.is_empty() {
                        results.push(SearchResult {
                            title: title.trim().to_string(),
                            url: raw_url.trim().to_string(),
                            snippet: snippet.trim().to_string(),
                        });
                    }
                }
            }
            cursor = pos + 20;
        }

        results
    }

    /// Search Google RSS for current affairs / news topics
    fn search_google_rss(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, String> {
        let url = format!(
            "https://news.google.com/rss/search?q={}&hl=en-US&gl=US&ceid=US:en",
            urlencoding::encode(query)
        );

        let response = self.client.get(&url)
            .send()
            .map_err(|e| format!("Google RSS request error: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Google RSS HTTP status {}", response.status()));
        }

        let xml = response.text().map_err(|e| format!("Failed to read RSS: {}", e))?;
        let mut results = Vec::new();
        let mut cursor = 0;

        while let Some(item_start) = xml[cursor..].find("<item>") {
            if results.len() >= limit {
                break;
            }
            let item_pos = cursor + item_start;
            let item_end = xml[item_pos..].find("</item>").unwrap_or(xml.len() - item_pos);
            let item_str = &xml[item_pos..item_pos + item_end];

            let title = Self::extract_tag_content(item_str, "title").unwrap_or_default();
            let link = Self::extract_tag_content(item_str, "link").unwrap_or_default();
            let snippet = Self::extract_tag_content(item_str, "description")
                .map(|d| Self::strip_html_tags(&d))
                .unwrap_or_default();

            if !title.is_empty() && !link.is_empty() && !title.contains("Google News") {
                results.push(SearchResult {
                    title: Self::decode_html_entities(&title),
                    url: link,
                    snippet: Self::decode_html_entities(&snippet),
                });
            }

            cursor = item_pos + item_end;
        }

        Ok(results)
    }

    /// Search Wikipedia API for encyclopedic and factual knowledge
    fn search_wikipedia(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, String> {
        let url = format!(
            "https://en.wikipedia.org/w/api.php?action=opensearch&search={}&limit={}&namespace=0&format=json",
            urlencoding::encode(query),
            limit
        );

        let response = self.client.get(&url)
            .send()
            .map_err(|e| format!("Wikipedia search request error: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Wikipedia HTTP status {}", response.status()));
        }

        let json: serde_json::Value = response.json()
            .map_err(|e| format!("Failed to parse Wikipedia JSON: {}", e))?;

        let mut results = Vec::new();
        if let (Some(titles), Some(snippets), Some(urls)) = (
            json.get(1).and_then(|v| v.as_array()),
            json.get(2).and_then(|v| v.as_array()),
            json.get(3).and_then(|v| v.as_array()),
        ) {
            for i in 0..titles.len().min(limit) {
                let title = titles[i].as_str().unwrap_or_default().to_string();
                let snippet = snippets.get(i).and_then(|s| s.as_str()).unwrap_or_default().to_string();
                let url = urls.get(i).and_then(|u| u.as_str()).unwrap_or_default().to_string();

                if !title.is_empty() && !url.is_empty() {
                    results.push(SearchResult { title, url, snippet });
                }
            }
        }

        Ok(results)
    }

    /// Fetch web page and distill clean, readable text stripped of tags, scripts, and styles
    pub fn fetch_text(&self, url: &str, max_chars: usize) -> Result<String, String> {
        let response = self.client.get(url)
            .send()
            .map_err(|e| format!("Failed to fetch URL '{}': {}", url, e))?;

        if !response.status().is_success() {
            return Err(format!("HTTP error {} fetching '{}'", response.status(), url));
        }

        let raw_html = response.text().map_err(|e| format!("Failed to read body: {}", e))?;
        let clean = Self::distill_html_to_text(&raw_html);

        if clean.len() > max_chars {
            Ok(format!("{}... [truncated]", &clean[..max_chars]))
        } else {
            Ok(clean)
        }
    }

    /// Convert raw HTML into clean readable text
    pub fn distill_html_to_text(html: &str) -> String {
        let mut clean = String::with_capacity(html.len() / 2);
        let mut in_script = false;
        let mut in_style = false;
        let mut in_tag = false;

        let lower = html.to_lowercase();
        let bytes = html.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            if !in_tag && bytes[i] == b'<' {
                if lower[i..].starts_with("<script") {
                    in_script = true;
                } else if lower[i..].starts_with("</script>") {
                    in_script = false;
                    i += 9;
                    continue;
                } else if lower[i..].starts_with("<style") {
                    in_style = true;
                } else if lower[i..].starts_with("</style>") {
                    in_style = false;
                    i += 8;
                    continue;
                }
                in_tag = true;
                i += 1;
                continue;
            }

            if in_tag {
                if bytes[i] == b'>' {
                    in_tag = false;
                    clean.push(' ');
                }
                i += 1;
                continue;
            }

            if in_script || in_style {
                i += 1;
                continue;
            }

            clean.push(bytes[i] as char);
            i += 1;
        }

        // Collapse multiple whitespace
        let decoded = Self::decode_html_entities(&clean);
        let mut result = String::new();
        let mut last_was_ws = false;

        for ch in decoded.chars() {
            if ch.is_whitespace() {
                if !last_was_ws {
                    result.push(' ');
                    last_was_ws = true;
                }
            } else {
                result.push(ch);
                last_was_ws = false;
            }
        }

        result.trim().to_string()
    }

    fn strip_html_tags(s: &str) -> String {
        let mut out = String::new();
        let mut in_tag = false;
        for c in s.chars() {
            if c == '<' {
                in_tag = true;
            } else if c == '>' {
                in_tag = false;
            } else if !in_tag {
                out.push(c);
            }
        }
        Self::decode_html_entities(&out)
    }

    fn extract_tag_content(xml: &str, tag: &str) -> Option<String> {
        let open_tag = format!("<{}>", tag);
        let close_tag = format!("</{}>", tag);
        if let Some(start) = xml.find(&open_tag) {
            let content_start = start + open_tag.len();
            if let Some(end) = xml[content_start..].find(&close_tag) {
                return Some(xml[content_start..content_start + end].to_string());
            }
        }
        None
    }

    fn decode_html_entities(s: &str) -> String {
        s.replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&#39;", "'")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&nbsp;", " ")
    }
}

// Simple standalone percent decoding helper
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut out = String::new();
        for b in s.bytes() {
            if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
                out.push(b as char);
            } else {
                out.push_str(&format!("%{:02X}", b));
            }
        }
        out
    }

    pub fn decode(s: &str) -> Result<String, ()> {
        let mut bytes = Vec::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '%' {
                let h1 = chars.next().ok_or(())?;
                let h2 = chars.next().ok_or(())?;
                let byte = u8::from_str_radix(&format!("{}{}", h1, h2), 16).map_err(|_| ())?;
                bytes.push(byte);
            } else if c == '+' {
                bytes.push(b' ');
            } else {
                bytes.push(c as u8);
            }
        }
        String::from_utf8(bytes).map_err(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distill_html_strips_scripts_and_tags() {
        let html = r#"
            <html>
                <head>
                    <title>Test Page</title>
                    <style>body { background: black; }</style>
                    <script>alert("hacked");</script>
                </head>
                <body>
                    <h1>Welcome &amp; Hello</h1>
                    <p>This is a <b>clean</b> paragraph with &quot;quotes&quot;.</p>
                </body>
            </html>
        "#;

        let clean = WebLens::distill_html_to_text(html);
        assert!(!clean.contains("alert"));
        assert!(!clean.contains("background: black"));
        assert!(clean.contains("Welcome & Hello"));
        assert!(clean.contains("This is a clean paragraph with \"quotes\"."));
    }

    #[test]
    fn test_url_encoding_decoding() {
        let original = "quantum computing & AI breakthroughs";
        let encoded = urlencoding::encode(original);
        assert!(encoded.contains("%20"));
        assert!(encoded.contains("%26"));

        let decoded = urlencoding::decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }
}
