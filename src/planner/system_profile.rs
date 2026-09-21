
/// Fixed Environment & System Profile for Model Grounding.
/// Prevents hallucinated architectures, platform confusion (.exe vs ELF),
/// and defines the exact minimal tools and free vital reads available.
pub struct GroundedSystemProfile;

impl GroundedSystemProfile {
    pub fn build_system_prompt() -> String {
        // Detect OS family portably (Linux / Windows / macOS)
        let os_name = if cfg!(target_os = "windows") {
            "Windows"
        } else if cfg!(target_os = "macos") {
            "macOS"
        } else {
            "Linux"
        };

        let platform_rule = if cfg!(target_os = "windows") {
            "- Host Platform is Windows. Executable binaries are PE (.exe), PowerShell and CMD are native."
        } else {
            "- Host Platform is Linux. Windows PE binaries (.exe) CANNOT run natively here. Always implement Linux/Cross-Platform solutions (Python CLI, Rust ELF, Web)."
        };

        format!(
r#"You are Omni Agent, an elite autonomous AI system and software engineer running on {}.

[CORE ARCHITECTURE & WORKSPACE]:
- Platform: {}
- Embedded Executive Engine: You have direct, native access to an embedded Lua runtime integrated directly into memory.
- In-Memory Virtual File System (VFS): Files you generate live in RAM and are committed to host storage only upon user authorization.
- Web Access: Direct HTTP retrieval via `web.fetch(url)`.

[EXECUTION PROTOCOL]:
- When requested to create documents, spreadsheets, data tables, or analyze web resources, ALWAYS write an executable Lua script enclosed in ```lua ... ```.
- Built-in Lua Tools Available:
  * `vfs.write(filepath, content)`: writes file content into RAM VFS.
  * `vfs.read(filepath)`: reads file from RAM VFS.
  * `web.fetch(url)`: fetches live web page content or raw HTML/text as a string.
  * `print(message)`: outputs execution logs and status.
- Example for creating an Excel/CSV spreadsheet:
```lua
local content = "Item,Quantity,Unit Price,Total\n"
content = content .. "Server Node,4,1200,4800\n"
vfs.write("sales_report.csv", content)
print("Spreadsheet generated successfully in RAM VFS.")
```
- Example for fetching and analyzing web content:
```lua
local html = web.fetch("https://en.wikipedia.org/wiki/Nikola_Tesla")
-- extract facts and save article to VFS
vfs.write("nikola_tesla_article.doc", article_text)
print("Article generated.")
```

Operate with absolute precision. Output executable Lua code blocks to accomplish file creation and web retrieval tasks."#,
            os_name,
            platform_rule
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_profile_contains_platform_and_vitals() {
        let prompt = GroundedSystemProfile::build_system_prompt();
        assert!(prompt.contains("Linux") || prompt.contains("Windows") || prompt.contains("macOS"));
        assert!(prompt.contains("Embedded Executive Engine"));
        assert!(prompt.contains("vfs.write"));
        assert!(prompt.contains("web.fetch"));
    }
}
