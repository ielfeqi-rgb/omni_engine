use ratatui::style::Color;
use std::path::{Path, PathBuf};
use std::io::Write;

pub struct OmniConfig {
    pub accent_color: Color,
    pub user_color: Color,
    pub ai_color: Color,
    pub default_model: String,
    pub context_size: usize,
    pub cpu_threads: usize,
    pub hardware_backend: String,
}

impl Default for OmniConfig {
    fn default() -> Self {
        Self {
            accent_color: Color::Cyan,
            user_color: Color::Green,
            ai_color: Color::Cyan,
            default_model: "qwen-0.5b-GGUF".to_string(),
            context_size: 8192,
            cpu_threads: 6,
            hardware_backend: "Vulkan".to_string(),
        }
    }
}

impl OmniConfig {
    pub fn config_dir() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("omni");
        let _ = std::fs::create_dir_all(&path);
        path
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("omni.conf")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if !path.exists() {
            Self::create_default_file(&path);
            return Self::default();
        }

        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let mut cfg = Self::default();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, val)) = line.split_once('=') {
                let key = key.trim().to_lowercase();
                let val = val.trim().trim_matches('"');
                match key.as_str() {
                    "accent_color" => if let Some(c) = Self::parse_color(val) { cfg.accent_color = c; },
                    "user_color" => if let Some(c) = Self::parse_color(val) { cfg.user_color = c; },
                    "ai_color" => if let Some(c) = Self::parse_color(val) { cfg.ai_color = c; },
                    "default_model" => cfg.default_model = val.to_string(),
                    "context_size" => if let Ok(n) = val.parse() { cfg.context_size = n; },
                    "cpu_threads" => if let Ok(n) = val.parse() { cfg.cpu_threads = n; },
                    "hardware_backend" => cfg.hardware_backend = val.to_string(),
                    _ => {}
                }
            }
        }
        cfg
    }

    fn create_default_file(path: &Path) {
        let default_content = r#"# ==========================================
# Omni Engine Configuration
# ==========================================

# Colors can be words (cyan, green, red, magenta) or HEX (#ff00ff)
accent_color = "cyan"
user_color = "green"
ai_color = "cyan"

# Engine
default_model = "qwen-0.5b-GGUF"
context_size = 8192
cpu_threads = 6
hardware_backend = "Vulkan"
"#;
        let _ = std::fs::File::create(path).and_then(|mut f| f.write_all(default_content.as_bytes()));
    }

    pub fn parse_color(s: &str) -> Option<Color> {
        let s = s.trim().to_lowercase();
        match s.as_str() {
            "cyan" => Some(Color::Cyan),
            "red" => Some(Color::Red),
            "green" => Some(Color::Green),
            "yellow" => Some(Color::Yellow),
            "blue" => Some(Color::Blue),
            "magenta" => Some(Color::Magenta),
            "gray" | "grey" => Some(Color::Gray),
            "darkgray" | "darkgrey" => Some(Color::DarkGray),
            "white" => Some(Color::White),
            "black" => Some(Color::Black),
            _ => {
                if s.starts_with('#') && s.len() == 7 {
                    let r = u8::from_str_radix(&s[1..3], 16).ok()?;
                    let g = u8::from_str_radix(&s[3..5], 16).ok()?;
                    let b = u8::from_str_radix(&s[5..7], 16).ok()?;
                    Some(Color::Rgb(r, g, b))
                } else {
                    None
                }
            }
        }
    }
}
