// ============================================================================
// [SOVEREIGN ENGINE CONSTITUTION - INVIOLABLE SAFETY DIRECTIVE]
// STRICT ARCHITECTURAL RULE:
// It is strictly forbidden, under any circumstances or for any reason whatsoever,
// to remove, comment out, bypass, or weaken `#![deny(unused_must_use)]` from
// this file or ANY other .rs file in this codebase.
// All fallible operations, Results, and Options must be explicitly handled.
// Zero silent error swallowing under the rug. This engine is built to endure for decades.
// ============================================================================
#![deny(unused_must_use)]

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use crate::sandbox::vfs::MemoryVfs;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiTestStep {
    pub action: String,
    pub selector: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiInspectionReport {
    pub passed: bool,
    pub exit_code: i32,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub dom_ids_found: Vec<String>,
    pub dom_classes_found: Vec<String>,
    pub steps_validated: usize,
}

impl UiInspectionReport {
    pub fn success(
        dom_ids: Vec<String>,
        dom_classes: Vec<String>,
        steps_validated: usize,
        warnings: Vec<String>,
    ) -> Self {
        Self {
            passed: true,
            exit_code: 0,
            errors: Vec::new(),
            warnings,
            dom_ids_found: dom_ids,
            dom_classes_found: dom_classes,
            steps_validated,
        }
    }

    pub fn failure(
        errors: Vec<String>,
        warnings: Vec<String>,
        dom_ids: Vec<String>,
        dom_classes: Vec<String>,
        steps_validated: usize,
    ) -> Self {
        Self {
            passed: false,
            exit_code: 1,
            errors,
            warnings,
            dom_ids_found: dom_ids,
            dom_classes_found: dom_classes,
            steps_validated,
        }
    }

    pub fn to_model_feedback(&self) -> String {
        if self.passed {
            let mut summary = format!(
                "✅ [UI_VERIFICATION_PASSED]: Exit Code 0. Validated {} test steps, {} DOM IDs, {} classes.",
                self.steps_validated,
                self.dom_ids_found.len(),
                self.dom_classes_found.len()
            );
            if !self.warnings.is_empty() {
                summary.push_str("\nWarnings:\n");
                for w in &self.warnings {
                    summary.push_str(&format!("  - {}\n", w));
                }
            }
            summary
        } else {
            let mut summary = format!(
                "❌ [UI_VERIFICATION_FAILED]: Exit Code 1. Found {} structural/contract error(s):\n",
                self.errors.len()
            );
            for (idx, err) in self.errors.iter().enumerate() {
                summary.push_str(&format!("{}. {}\n", idx + 1, err));
            }
            if !self.warnings.is_empty() {
                summary.push_str("\nWarnings:\n");
                for w in &self.warnings {
                    summary.push_str(&format!("  - {}\n", w));
                }
            }
            summary.push_str("\nAction Required: Correct the HTML elements, JavaScript DOM queries, or test step selectors to restore exact contract parity.");
            summary
        }
    }
}

/// Native Deterministic UI & Web Contract Verifier
pub struct UiContractVerifier;

impl UiContractVerifier {
    /// Extract DOM metadata (IDs, classes, tag names, embedded scripts) from HTML text
    pub fn extract_dom_metadata(
        html: &str,
    ) -> (
        HashSet<String>,
        HashSet<String>,
        HashSet<String>,
        Vec<String>,
        Vec<String>,
    ) {
        let mut ids = HashSet::new();
        let mut classes = HashSet::new();
        let mut tags = HashSet::new();
        let mut embedded_scripts = Vec::new();
        let mut warnings = Vec::new();

        // Check for viewport meta tag
        if !html.contains("name=\"viewport\"") && !html.contains("name='viewport'") {
            warnings.push("HTML document is missing `<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">`. Mobile responsiveness may be impaired.".to_string());
        }

        // 1. Extract embedded <script> tags
        let mut script_cursor = 0;
        while let Some(start_rel) = html[script_cursor..].find("<script") {
            let tag_start = script_cursor + start_rel;
            if let Some(tag_close_rel) = html[tag_start..].find('>') {
                let body_start = tag_start + tag_close_rel + 1;
                if let Some(end_rel) = html[body_start..].find("</script>") {
                    let body_end = body_start + end_rel;
                    let script_body = html[body_start..body_end].trim();
                    if !script_body.is_empty() {
                        embedded_scripts.push(script_body.to_string());
                    }
                    script_cursor = body_end + 9;
                } else {
                    script_cursor = body_start;
                }
            } else {
                script_cursor = tag_start + 7;
            }
        }

        // 2. Tokenize elements and parse attributes
        let mut cursor = 0;
        while let Some(start_rel) = html[cursor..].find('<') {
            let start = cursor + start_rel;
            if start + 1 >= html.len() {
                break;
            }

            // Skip comments and closing tags
            if html[start..].starts_with("<!--") {
                if let Some(comment_end) = html[start..].find("-->") {
                    cursor = start + comment_end + 3;
                    continue;
                } else {
                    break;
                }
            }

            let is_closing = html[start..].starts_with("</");
            let tag_content_start = if is_closing { start + 2 } else { start + 1 };

            if let Some(end_rel) = html[tag_content_start..].find('>') {
                let tag_str = &html[tag_content_start..tag_content_start + end_rel].trim();
                cursor = tag_content_start + end_rel + 1;

                if tag_str.is_empty() {
                    continue;
                }

                let first_space = tag_str.find(char::is_whitespace).unwrap_or(tag_str.len());
                let tag_name = tag_str[..first_space].to_ascii_lowercase();

                if !is_closing && !tag_name.is_empty() {
                    tags.insert(tag_name.clone());
                    Self::parse_element_attributes(tag_str, &mut ids, &mut classes);
                }
            } else {
                break;
            }
        }

        (ids, classes, tags, embedded_scripts, warnings)
    }

    fn parse_element_attributes(
        tag_str: &str,
        ids: &mut HashSet<String>,
        classes: &mut HashSet<String>,
    ) {
        // Extract id="value" or id='value'
        Self::extract_attr_values(tag_str, "id", ids);

        // Extract class="val1 val2"
        let mut class_strings = HashSet::new();
        Self::extract_attr_values(tag_str, "class", &mut class_strings);
        for full_cls in class_strings {
            for token in full_cls.split_whitespace() {
                let trimmed = token.trim();
                if !trimmed.is_empty() {
                    classes.insert(trimmed.to_string());
                }
            }
        }
    }

    fn extract_attr_values(tag_str: &str, attr_name: &str, sink: &mut HashSet<String>) {
        let patterns = [
            format!("{}=\"", attr_name),
            format!("{}='", attr_name),
            format!("{} =\"", attr_name),
            format!("{} ='", attr_name),
        ];

        for pattern in &patterns {
            let mut search_idx = 0;
            while let Some(rel) = tag_str[search_idx..].find(pattern.as_str()) {
                let start = search_idx + rel + pattern.len();
                let quote_char = pattern.chars().last().unwrap_or('"');
                if let Some(end_rel) = tag_str[start..].find(quote_char) {
                    let val = &tag_str[start..start + end_rel].trim();
                    if !val.is_empty() {
                        sink.insert(val.to_string());
                    }
                    search_idx = start + end_rel + 1;
                } else {
                    break;
                }
            }
        }
    }

    /// Validate test_steps.json actions against extracted DOM symbols
    pub fn validate_test_steps(
        steps_json: &str,
        dom_ids: &HashSet<String>,
        dom_classes: &HashSet<String>,
        dom_tags: &HashSet<String>,
    ) -> Result<usize, Vec<String>> {
        let steps: Vec<UiTestStep> = match serde_json::from_str(steps_json) {
            Ok(s) => s,
            Err(e) => {
                return Err(vec![format!(
                    "[UI_TEST_STEPS_INVALID]: Failed to parse test_steps.json as JSON: {}",
                    e
                )]);
            }
        };

        if steps.is_empty() {
            return Err(vec!["[UI_TEST_STEPS_EMPTY]: test_steps.json contains zero test actions.".to_string()]);
        }

        let mut errors = Vec::new();
        for (i, step) in steps.iter().enumerate() {
            let sel = step.selector.trim();
            if sel.is_empty() {
                errors.push(format!("Step {} (action '{}') defines an empty selector.", i + 1, step.action));
                continue;
            }

            let mut matched = false;

            // Handle ID selector: #some-id
            if let Some(id_part) = sel.strip_prefix('#') {
                if dom_ids.contains(id_part) {
                    matched = true;
                }
            }
            // Handle Class selector: .some-class
            else if let Some(class_part) = sel.strip_prefix('.') {
                if dom_classes.contains(class_part) {
                    matched = true;
                }
            }
            // Handle raw tag selector: button, input, div
            else if dom_tags.contains(&sel.to_ascii_lowercase()) {
                matched = true;
            }
            // Compound or attribute selector fallback: e.g. input[type="text"] or button#submit
            else {
                for id in dom_ids {
                    if sel.contains(id.as_str()) {
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    for cls in dom_classes {
                        if sel.contains(cls.as_str()) {
                            matched = true;
                            break;
                        }
                    }
                }
                if !matched {
                    for tag in dom_tags {
                        if sel.starts_with(tag.as_str()) {
                            matched = true;
                            break;
                        }
                    }
                }
            }

            if !matched {
                errors.push(format!(
                    "[UI_CONTRACT_ERROR]: Test step {} specifies action '{}' on selector '{}', but no matching element (id, class, or tag) was found in HTML.",
                    i + 1,
                    step.action,
                    step.selector
                ));
            }
        }

        if errors.is_empty() {
            Ok(steps.len())
        } else {
            Err(errors)
        }
    }

    /// Audit JavaScript DOM lookup contracts & bracket balance
    pub fn validate_js_dom_contracts(js_content: &str, dom_ids: &HashSet<String>) -> Vec<String> {
        let mut errors = Vec::new();

        // 1. Bracket & brace balance check
        let mut paren_balance: i32 = 0;
        let mut brace_balance: i32 = 0;
        let mut bracket_balance: i32 = 0;
        let mut in_string = false;
        let mut string_char = ' ';
        let mut escaped = false;

        for ch in js_content.chars() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == string_char {
                    in_string = false;
                }
                continue;
            }

            if ch == '\'' || ch == '"' || ch == '`' {
                in_string = true;
                string_char = ch;
                continue;
            }

            match ch {
                '(' => paren_balance += 1,
                ')' => paren_balance -= 1,
                '{' => brace_balance += 1,
                '}' => brace_balance -= 1,
                '[' => bracket_balance += 1,
                ']' => bracket_balance -= 1,
                _ => {}
            }

            if paren_balance < 0 {
                errors.push("[JS_SYNTAX_ERROR]: Unexpected closing parenthesis ')' in JavaScript without matching opening '('. ".to_string());
                break;
            }
            if brace_balance < 0 {
                errors.push("[JS_SYNTAX_ERROR]: Unexpected closing brace '}' in JavaScript without matching opening '{'.".to_string());
                break;
            }
            if bracket_balance < 0 {
                errors.push("[JS_SYNTAX_ERROR]: Unexpected closing bracket ']' in JavaScript without matching opening '['.".to_string());
                break;
            }
        }

        if paren_balance > 0 {
            errors.push(format!("[JS_SYNTAX_ERROR]: Unclosed parenthesis '(' in JavaScript ({} unclosed).", paren_balance));
        }
        if brace_balance > 0 {
            errors.push(format!("[JS_SYNTAX_ERROR]: Unclosed brace '{{' in JavaScript ({} unclosed).", brace_balance));
        }
        if bracket_balance > 0 {
            errors.push(format!("[JS_SYNTAX_ERROR]: Unclosed square bracket '[' in JavaScript ({} unclosed).", bracket_balance));
        }

        // 2. Scan getElementById & querySelector for contract violations
        let patterns = [
            "getElementById('",
            "getElementById(\"",
            "getElementById(`",
            "querySelector('#",
            "querySelector(\"#",
            "querySelector(`#",
        ];

        for pattern in &patterns {
            let mut search_idx = 0;
            while let Some(rel) = js_content[search_idx..].find(pattern) {
                let start = search_idx + rel + pattern.len();
                let close_delim = if pattern.contains('\'') {
                    '\''
                } else if pattern.contains('\"') {
                    '\"'
                } else {
                    '`'
                };

                if let Some(end_rel) = js_content[start..].find(close_delim) {
                    let id_accessed = &js_content[start..start + end_rel].trim();
                    if !id_accessed.is_empty() && !dom_ids.contains(*id_accessed) {
                        errors.push(format!(
                            "[JS_DOM_MISMATCH]: JavaScript accesses element by ID '{}', but no element with id='{}' exists in HTML.",
                            id_accessed,
                            id_accessed
                        ));
                    }
                    search_idx = start + end_rel + 1;
                } else {
                    break;
                }
            }
        }

        errors
    }

    /// Audit Frontend fetch API endpoints against Backend Python routes
    pub fn validate_api_contracts(js_content: &str, backend_content: &str) -> Vec<String> {
        let mut errors = Vec::new();

        // 1. Extract fetch(...) URL endpoints
        let fetch_patterns = ["fetch('", "fetch(\"", "fetch(`"];
        let mut js_endpoints = Vec::new();

        for pattern in &fetch_patterns {
            let mut search_idx = 0;
            while let Some(rel) = js_content[search_idx..].find(pattern) {
                let start = search_idx + rel + pattern.len();
                let close_delim = if pattern.contains('\'') {
                    '\''
                } else if pattern.contains('\"') {
                    '\"'
                } else {
                    '`'
                };

                if let Some(end_rel) = js_content[start..].find(close_delim) {
                    let mut endpoint = js_content[start..start + end_rel].trim().to_string();
                    if let Some(q_idx) = endpoint.find('?') {
                        endpoint = endpoint[..q_idx].to_string();
                    }
                    if endpoint.starts_with('/') {
                        js_endpoints.push(endpoint);
                    }
                    search_idx = start + end_rel + 1;
                } else {
                    break;
                }
            }
        }

        // 2. Check if backend contains matching endpoints or parent resource
        for ep in &js_endpoints {
            let mut matches = backend_content.contains(ep.as_str());

            // If not directly found, check if it is a dynamic path like /api/tasks/123 -> check /api/tasks
            if !matches {
                let segments: Vec<&str> = ep.split('/').filter(|s| !s.is_empty()).collect();
                if segments.len() > 2 {
                    let base_path = format!("/{}", segments[..segments.len() - 1].join("/"));
                    if backend_content.contains(&base_path) {
                        matches = true;
                    }
                }
            }

            if !matches {
                errors.push(format!(
                    "[API_CONTRACT_MISMATCH]: JavaScript calls endpoint '{}' via fetch(), but backend does not define a route matching this endpoint.",
                    ep
                ));
            }
        }

        errors
    }

    /// Verify full UI Artifact suite
    pub fn verify_ui_artifact(
        html_content: &str,
        external_js: Option<&str>,
        steps_json: Option<&str>,
        backend_content: Option<&str>,
    ) -> UiInspectionReport {
        let (dom_ids, dom_classes, dom_tags, embedded_scripts, warnings) =
            Self::extract_dom_metadata(html_content);

        let mut all_errors = Vec::new();
        let mut steps_validated = 0;

        // 1. Validate test_steps.json if provided
        if let Some(steps) = steps_json {
            match Self::validate_test_steps(steps, &dom_ids, &dom_classes, &dom_tags) {
                Ok(count) => steps_validated = count,
                Err(errs) => all_errors.extend(errs),
            }
        }

        // 2. Combine JS sources (embedded + external)
        let mut combined_js = embedded_scripts.join("\n;\n");
        if let Some(ext_js) = external_js {
            if !combined_js.is_empty() {
                combined_js.push_str("\n;\n");
            }
            combined_js.push_str(ext_js);
        }

        // 3. Validate JS contracts
        if !combined_js.trim().is_empty() {
            let js_errs = Self::validate_js_dom_contracts(&combined_js, &dom_ids);
            all_errors.extend(js_errs);

            // 4. Validate API contracts if backend code exists
            if let Some(backend) = backend_content {
                let api_errs = Self::validate_api_contracts(&combined_js, backend);
                all_errors.extend(api_errs);
            }
        }

        let mut sorted_ids: Vec<String> = dom_ids.into_iter().collect();
        sorted_ids.sort();
        let mut sorted_classes: Vec<String> = dom_classes.into_iter().collect();
        sorted_classes.sort();

        if all_errors.is_empty() {
            UiInspectionReport::success(sorted_ids, sorted_classes, steps_validated, warnings)
        } else {
            UiInspectionReport::failure(all_errors, warnings, sorted_ids, sorted_classes, steps_validated)
        }
    }

    /// Inspect UI artifacts directly from MemoryVfs
    pub fn verify_vfs_ui_artifacts(vfs: &MemoryVfs) -> Option<UiInspectionReport> {
        let files = vfs.list_files();
        let html_file = files.iter().find(|p| p.to_string_lossy().ends_with(".html"))?;
        let html_bytes = vfs.read_file(html_file)?;
        let html_content = String::from_utf8(html_bytes).ok()?;

        // Look for external js
        let js_content = files
            .iter()
            .find(|p| {
                let s = p.to_string_lossy();
                s.ends_with(".js") && !s.ends_with("tester.js") && !s.ends_with("inspector.js")
            })
            .and_then(|p| vfs.read_file(p))
            .and_then(|b| String::from_utf8(b).ok());

        // Look for test_steps.json
        let steps_json = files
            .iter()
            .find(|p| p.to_string_lossy().ends_with("test_steps.json"))
            .and_then(|p| vfs.read_file(p))
            .and_then(|b| String::from_utf8(b).ok());

        // Look for backend python files
        let backend_content = files
            .iter()
            .find(|p| {
                let s = p.to_string_lossy();
                s.ends_with(".py") && !s.contains("test_")
            })
            .and_then(|p| vfs.read_file(p))
            .and_then(|b| String::from_utf8(b).ok());

        Some(Self::verify_ui_artifact(
            &html_content,
            js_content.as_deref(),
            steps_json.as_deref(),
            backend_content.as_deref(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_ui_artifact_passes() {
        let html = r#"<!DOCTYPE html>
<html>
<head>
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Login</title>
</head>
<body>
    <form id="login-form">
        <input id="email" type="email" class="input-field" />
        <button id="login-btn" class="btn primary">Login</button>
    </form>
    <div id="success-msg" class="hidden">Welcome</div>
</body>
</html>"#;

        let js = r#"
document.getElementById('login-btn').addEventListener('click', function(e) {
    const email = document.getElementById('email').value;
    document.getElementById('success-msg').textContent = 'Success ' + email;
});
"#;

        let steps = r##"[
            {"action": "type", "selector": "#email", "value": "user@example.com"},
            {"action": "click", "selector": "#login-btn"},
            {"action": "wait_for", "selector": "#success-msg"}
        ]"##;

        let report = UiContractVerifier::verify_ui_artifact(html, Some(js), Some(steps), None);
        assert!(report.passed);
        assert_eq!(report.exit_code, 0);
        assert_eq!(report.steps_validated, 3);
        assert!(report.dom_ids_found.contains(&"email".to_string()));
        assert!(report.dom_ids_found.contains(&"login-btn".to_string()));
        assert!(report.dom_ids_found.contains(&"success-msg".to_string()));
    }

    #[test]
    fn test_missing_selector_in_test_steps_fails() {
        let html = r#"<html><head><meta name="viewport" content="width=device-width"></head><body><div id="content"></div></body></html>"#;
        let steps = r##"[{"action": "click", "selector": "#non-existent-button"}]"##;

        let report = UiContractVerifier::verify_ui_artifact(html, None, Some(steps), None);
        assert!(!report.passed);
        assert_eq!(report.exit_code, 1);
        assert!(report.errors[0].contains("#non-existent-button"));
    }

    #[test]
    fn test_js_dom_mismatch_fails() {
        let html = r#"<html><head><meta name="viewport" content="width=device-width"></head><body><input id="user_email"></body></html>"#;
        let js = r#"const input = document.getElementById('user-email');"#;

        let report = UiContractVerifier::verify_ui_artifact(html, Some(js), None, None);
        assert!(!report.passed);
        assert_eq!(report.exit_code, 1);
        assert!(report.errors[0].contains("user-email"));
    }

    #[test]
    fn test_js_unbalanced_brackets_fails() {
        let html = r#"<html><head><meta name="viewport" content="width=device-width"></head><body><div id="box"></div></body></html>"#;
        let js = r#"function update() { document.getElementById('box').textContent = 'ok';"#; // Missing closing brace

        let report = UiContractVerifier::verify_ui_artifact(html, Some(js), None, None);
        assert!(!report.passed);
        assert_eq!(report.exit_code, 1);
        assert!(report.errors.iter().any(|e| e.contains("[JS_SYNTAX_ERROR]")));
    }

    #[test]
    fn test_api_route_mismatch_fails() {
        let html = r#"<html><head><meta name="viewport" content="width=device-width"></head><body><div id="box"></div></body></html>"#;
        let js = r#"fetch('/api/tasks').then(r => r.json());"#;
        let backend = r#"
@app.route('/api/items')
def get_items():
    return []
"#;

        let report = UiContractVerifier::verify_ui_artifact(html, Some(js), None, Some(backend));
        assert!(!report.passed);
        assert_eq!(report.exit_code, 1);
        assert!(report.errors[0].contains("/api/tasks"));
    }

    #[test]
    fn test_vfs_ui_verification() {
        let vfs = MemoryVfs::new();
        vfs.write_file(
            "index.html",
            r#"<!DOCTYPE html><html><head><meta name="viewport" content="width=device-width"></head><body><button id="btn">Click</button></body></html>"#.as_bytes(),
        );
        vfs.write_file(
            "test_steps.json",
            r##"[{"action": "click", "selector": "#btn"}]"##.as_bytes(),
        );

        let report = UiContractVerifier::verify_vfs_ui_artifacts(&vfs);
        assert!(report.is_some());
        let rep = report.unwrap();
        assert!(rep.passed);
        assert_eq!(rep.exit_code, 0);
        assert_eq!(rep.steps_validated, 1);
    }
}
