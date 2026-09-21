use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ActionTargetType {
    Button,
    Input,
    Link,
    SelectableRow,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveElement {
    pub id: usize,
    pub target_type: ActionTargetType,
    pub label: String,
    pub selector: String,
    pub current_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalPageProjection {
    pub title: String,
    pub url: String,
    pub ascii_grid: String,
    pub indexed_elements: HashMap<usize, InteractiveElement>,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentBrowserAction {
    Click { target_id: usize },
    Type { target_id: usize, text: String },
    Navigate { url: String },
    ScrollDown,
    RequestUserConfirmation { action_description: String },
}

/// The Intermediate Browser Lens and Translator.
/// Converts dense HTML/DOM elements into a lightweight 2D Terminal ASCII Canvas
/// with unique integer index tags: [1], [2], [3]...
/// The small model in the Sandbox outputs ONLY numeric actions (e.g. click 2),
/// completely decoupled from low-level web vulnerabilities.
pub struct BrowserTerminalLens {
    cols: usize,
    rows: usize,
}

impl BrowserTerminalLens {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self { cols, rows }
    }

    /// Project raw list of interactive elements into a clean 2D terminal grid string
    pub fn project_canvas(
        &self,
        title: &str,
        url: &str,
        elements: Vec<InteractiveElement>,
    ) -> TerminalPageProjection {
        let mut grid: Vec<Vec<char>> = vec![vec![' '; self.cols]; self.rows];
        let mut indexed_map = HashMap::new();

        // 1. Render Header Bar
        let header = format!("┌─ OMNI BROWSER LENS: {} ({}) ─┐", title, url);
        self.write_to_grid(&mut grid, 0, 0, &header);

        // 2. Render Elements into grid lines
        let mut current_row = 2;
        for el in elements {
            if current_row >= self.rows - 1 {
                break;
            }

            let type_symbol = match el.target_type {
                ActionTargetType::Button => "🔘 [BTN]",
                ActionTargetType::Input => "📝 [INPUT]",
                ActionTargetType::Link => "🔗 [LINK]",
                ActionTargetType::SelectableRow => "📧 [ROW]",
                ActionTargetType::Custom => "🔹 [ITEM]",
            };

            let line_str = match &el.current_value {
                Some(val) => format!("[{}] {} {} -> \"{}\"", el.id, type_symbol, el.label, val),
                None => format!("[{}] {} {}", el.id, type_symbol, el.label),
            };

            self.write_to_grid(&mut grid, 2, current_row, &line_str);
            indexed_map.insert(el.id, el);
            current_row += 1;
        }

        // 3. Render Footer
        let footer = "└─────────────────────────────────────────────────────────────┘";
        self.write_to_grid(&mut grid, 0, self.rows - 1, footer);

        let ascii_grid: String = grid
            .iter()
            .map(|row| row.iter().collect::<String>().trim_end().to_string())
            .collect::<Vec<String>>()
            .join("\n");

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        TerminalPageProjection {
            title: title.to_string(),
            url: url.to_string(),
            ascii_grid,
            indexed_elements: indexed_map,
            timestamp_ms: now,
        }
    }

    /// Resolve an agent's numeric decision back to real DOM selector
    pub fn resolve_action(
        &self,
        projection: &TerminalPageProjection,
        action: AgentBrowserAction,
    ) -> Result<String, String> {
        match action {
            AgentBrowserAction::Click { target_id } => {
                let elem = projection
                    .indexed_elements
                    .get(&target_id)
                    .ok_or_else(|| format!("Invalid target_id [{}] not found in current canvas", target_id))?;
                Ok(format!("CLICK: selector='{}' (label='{}')", elem.selector, elem.label))
            }
            AgentBrowserAction::Type { target_id, text } => {
                let elem = projection
                    .indexed_elements
                    .get(&target_id)
                    .ok_or_else(|| format!("Invalid target_id [{}] not found in current canvas", target_id))?;
                Ok(format!("TYPE: selector='{}' text='{}'", elem.selector, text))
            }
            AgentBrowserAction::Navigate { url } => Ok(format!("NAVIGATE: url='{}'", url)),
            AgentBrowserAction::ScrollDown => Ok("SCROLL_DOWN".to_string()),
            AgentBrowserAction::RequestUserConfirmation { action_description } => {
                Ok(format!("PROMPT_USER: '{}'", action_description))
            }
        }
    }

    fn write_to_grid(&self, grid: &mut [Vec<char>], start_col: usize, row: usize, text: &str) {
        if row >= self.rows {
            return;
        }
        let chars: Vec<char> = text.chars().collect();
        for (i, &ch) in chars.iter().enumerate() {
            let col = start_col + i;
            if col < self.cols {
                grid[row][col] = ch;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_lens_projection_and_resolution() {
        let lens = BrowserTerminalLens::new(80, 10);

        let elements = vec![
            InteractiveElement {
                id: 1,
                target_type: ActionTargetType::Input,
                label: "Search Input".to_string(),
                selector: "#mail-search".to_string(),
                current_value: Some("".to_string()),
            },
            InteractiveElement {
                id: 2,
                target_type: ActionTargetType::SelectableRow,
                label: "Airport Flight Departure Confirmation".to_string(),
                selector: "#row-flight-101".to_string(),
                current_value: None,
            },
            InteractiveElement {
                id: 3,
                target_type: ActionTargetType::Button,
                label: "Send Transfer".to_string(),
                selector: "#btn-transfer".to_string(),
                current_value: None,
            },
        ];

        let projection = lens.project_canvas("Webmail App", "http://localhost/mail", elements);

        // Verification 1: Terminal grid contains the index identifiers
        assert!(projection.ascii_grid.contains("[1]"));
        assert!(projection.ascii_grid.contains("[2]"));
        assert!(projection.ascii_grid.contains("[3]"));
        assert!(projection.ascii_grid.contains("Webmail App"));

        // Verification 2: Model decides to click flight email [2]
        let click_action = AgentBrowserAction::Click { target_id: 2 };
        let resolved = lens.resolve_action(&projection, click_action).unwrap();
        assert_eq!(resolved, "CLICK: selector='#row-flight-101' (label='Airport Flight Departure Confirmation')");

        // Verification 3: Invalid index trapped cleanly
        let invalid_action = AgentBrowserAction::Click { target_id: 99 };
        assert!(lens.resolve_action(&projection, invalid_action).is_err());
    }
}
