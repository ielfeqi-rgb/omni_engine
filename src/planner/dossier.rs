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

//! Customer Dossier & Active Epistemic Distillation Module.
//!
//! # Architecture:
//! - Rather than naive context bloating (dumping 20-50 raw chat turns into the KV-cache),
//!   Omni Engine passes raw interactions through the System 2 Planner.
//! - Non-essential pleasantries are discarded; high-signal entities (identity, sizes, materials,
//!   exact purchase SKUs, open questions) are distilled into an on-disk JSON dossier.
//! - Subsequent sessions inject ONLY a ~30-50 token Epistemic Anchor into the System Prompt.
//! - At session teardown, the active sequence's KV-cache slot is wiped from RAM (`kv_cache_seq_rm`),
//!   maintaining minimal memory footprint and zero latency.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;
use crate::openai_api::ChatMessage;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PurchaseRecord {
    pub sku: String,
    pub name: String,
    pub price: f64,
    pub quantity: u32,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DistillationDelta {
    pub name: Option<String>,
    pub city: Option<String>,
    pub preferences: HashMap<String, String>,
    pub new_purchases: Vec<PurchaseRecord>,
    pub unresolved_queries: Vec<String>,
    pub distilled_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomerDossier {
    pub phone: String,
    pub name: Option<String>,
    pub city: Option<String>,
    pub preferences: HashMap<String, String>,
    pub purchases: Vec<PurchaseRecord>,
    pub unresolved_queries: Vec<String>,
    pub distilled_summary: String,
    pub visit_count: usize,
    pub first_seen: String,
    pub last_seen: String,
}

impl CustomerDossier {
    pub fn new(phone: impl Into<String>) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            phone: phone.into(),
            name: None,
            city: None,
            preferences: HashMap::new(),
            purchases: Vec::new(),
            unresolved_queries: Vec::new(),
            distilled_summary: String::new(),
            visit_count: 1,
            first_seen: now.clone(),
            last_seen: now,
        }
    }

    /// Resolves the filesystem path for a customer dossier JSON file.
    pub fn file_path(base_dir: &Path, phone: &str) -> PathBuf {
        let clean_phone: String = phone.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect();
        base_dir.join("data").join("customers").join(format!("{}.json", clean_phone))
    }

    /// Loads a dossier from disk or initializes a new one if not found.
    pub fn load_or_create(base_dir: &Path, phone: &str) -> Result<Self, String> {
        let path = Self::file_path(base_dir, phone);
        if path.exists() {
            let content = fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read customer dossier at {:?}: {}", path, e))?;
            let mut dossier: CustomerDossier = serde_json::from_str(&content)
                .map_err(|e| format!("Corrupt customer dossier JSON at {:?}: {}", path, e))?;
            dossier.visit_count += 1;
            dossier.last_seen = chrono::Utc::now().to_rfc3339();
            Ok(dossier)
        } else {
            Ok(Self::new(phone))
        }
    }

    /// Safely writes the dossier to disk using atomic two-phase write.
    pub fn save_to_disk(&self, base_dir: &Path) -> Result<(), String> {
        let target_dir = base_dir.join("data").join("customers");
        if let Err(e) = fs::create_dir_all(&target_dir) {
            return Err(format!("Failed to create customer dossier directory {:?}: {}", target_dir, e));
        }

        let target_file = Self::file_path(base_dir, &self.phone);
        let temp_file = target_dir.join(format!(".tmp_{}_{}", self.phone, uuid::Uuid::new_v4().simple()));

        let json_bytes = serde_json::to_vec_pretty(self)
            .map_err(|e| format!("Failed to serialize dossier: {}", e))?;

        if let Err(e) = fs::write(&temp_file, &json_bytes) {
            return Err(format!("Failed to write temp dossier {:?}: {}", temp_file, e));
        }

        if let Err(e) = fs::rename(&temp_file, &target_file) {
            let _ = fs::remove_file(&temp_file);
            return Err(format!("Atomic rename failed from {:?} to {:?}: {}", temp_file, target_file, e));
        }

        info!("Customer dossier successfully persisted to {:?}", target_file);
        Ok(())
    }

    /// Formats the compact Epistemic Anchor to inject into the System Prompt.
    /// This costs typically ~30-50 tokens, eliminating the 2,000-4,000 token overhead
    /// of raw chat histories.
    pub fn format_epistemic_anchor(&self) -> String {
        let mut parts = Vec::new();

        if let Some(ref name) = self.name {
            parts.push(format!("Name: {}", name));
        }
        parts.push(format!("Phone: {}", self.phone));
        if let Some(ref city) = self.city {
            parts.push(format!("City: {}", city));
        }

        if !self.preferences.is_empty() {
            let pref_str: Vec<String> = self.preferences
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            parts.push(format!("Preferences: [{}]", pref_str.join(", ")));
        }

        if !self.purchases.is_empty() {
            let purchases_summary: Vec<String> = self.purchases
                .iter()
                .map(|p| format!("{} (SKU:{})", p.name, p.sku))
                .collect();
            parts.push(format!("Past Orders: [{}]", purchases_summary.join(", ")));
        }

        if !self.distilled_summary.is_empty() {
            parts.push(format!("Summary: {}", self.distilled_summary));
        }

        if !self.unresolved_queries.is_empty() {
            parts.push(format!("Pending: [{}]", self.unresolved_queries.join("; ")));
        }

        format!("\n[Customer Epistemic Anchor: {}]\n", parts.join(" | "))
    }

    /// Merges an extracted DistillationDelta into this long-term profile.
    pub fn apply_delta(&mut self, delta: DistillationDelta) {
        if let Some(name) = delta.name {
            if !name.trim().is_empty() {
                self.name = Some(name);
            }
        }
        if let Some(city) = delta.city {
            if !city.trim().is_empty() {
                self.city = Some(city);
            }
        }
        for (k, v) in delta.preferences {
            if !v.trim().is_empty() {
                self.preferences.insert(k, v);
            }
        }
        for order in delta.new_purchases {
            if !self.purchases.iter().any(|p| p.sku == order.sku && p.timestamp == order.timestamp) {
                self.purchases.push(order);
            }
        }
        if !delta.distilled_summary.trim().is_empty() {
            self.distilled_summary = delta.distilled_summary;
        }
        self.unresolved_queries = delta.unresolved_queries;
        self.last_seen = chrono::Utc::now().to_rfc3339();
    }
}

/// The System 2 Epistemic Distiller.
/// Condenses verbose chat sessions (10-50 turns) into compact, high-signal structured profile deltas.
pub struct PlannerDistiller;

impl PlannerDistiller {
    /// Builds the distillation prompt instructing the Planner model to extract hard facts
    /// and discard polite filler.
    pub fn build_distillation_prompt(
        messages: &[ChatMessage],
        existing_profile: Option<&CustomerDossier>,
    ) -> String {
        let mut transcript = String::new();
        for msg in messages {
            let content = msg.content.as_deref().unwrap_or("");
            if !content.is_empty() {
                transcript.push_str(&format!("{}: {}\n", msg.role, content));
            }
        }

        let existing_context = if let Some(p) = existing_profile {
            format!(
                "Existing Profile:\n- Name: {:?}\n- City: {:?}\n- Known Preferences: {:?}\n- Previous Purchases: {:?}\n",
                p.name, p.city, p.preferences, p.purchases
            )
        } else {
            "No prior profile exists for this customer.\n".to_string()
        };

        format!(
            "You are Omni Engine System 2 Epistemic Distiller.\n\
            Your duty is to compress raw customer conversation into high-signal truth.\n\
            DISCARD all pleasantries, greetings, apologies, and chatter.\n\
            EXTRACT only definitive facts:\n\
            - Customer Name, City\n\
            - Preferences (sizes, fabrics, favorite colors, fit)\n\
            - Definite purchases or orders made (SKU, name, price, quantity)\n\
            - Unresolved inquiries or questions the customer left open\n\
            - A dense, 1-2 sentence behavioral summary\n\n\
            {}\n\
            Conversation Transcript:\n\
            {}\n\n\
            Output STRICTLY valid JSON matching this schema:\n\
            {{\n\
              \"name\": \"string or null\",\n\
              \"city\": \"string or null\",\n\
              \"preferences\": {{ \"key\": \"value\" }},\n\
              \"new_purchases\": [\n\
                {{ \"sku\": \"exact_sku\", \"name\": \"item_name\", \"price\": 0.0, \"quantity\": 1, \"timestamp\": \"ISO8601\" }}\n\
              ],\n\
              \"unresolved_queries\": [\"string\"],\n\
              \"distilled_summary\": \"compact summary\"\n\
            }}",
            existing_context, transcript
        )
    }

    /// Parses the JSON response produced by the Planner model.
    pub fn parse_distillation_response(response: &str) -> Result<DistillationDelta, String> {
        let json_start = response.find('{').ok_or_else(|| "No JSON object found in distillation output".to_string())?;
        let json_end = response.rfind('}').ok_or_else(|| "No closing brace found in distillation output".to_string())?;

        let json_str = &response[json_start..=json_end];
        serde_json::from_str::<DistillationDelta>(json_str)
            .map_err(|e| format!("Failed to parse distillation delta JSON: {}", e))
    }

    /// Fast rule-based heuristic extractor used as an ultra-fast offline fallback.
    /// Extracts size, phone, and purchase hints without needing an LLM generation cycle.
    pub fn heuristic_distill(messages: &[ChatMessage]) -> DistillationDelta {
        let mut delta = DistillationDelta::default();
        let mut summary_points = Vec::new();

        for msg in messages {
            let content = match msg.content.as_deref() {
                Some(c) => c,
                None => continue,
            };

            let lower = content.to_lowercase();

            // Extract sizes
            for size in &["xxl", "xl", " l ", " m ", " s "] {
                if lower.contains(size) {
                    let clean_size = size.trim().to_uppercase();
                    delta.preferences.insert("size".to_string(), clean_size.clone());
                    summary_points.push(format!("size {}", clean_size));
                    break;
                }
            }

            // Extract material hints
            if lower.contains("قطن") || lower.contains("cotton") {
                delta.preferences.insert("fabric".to_string(), "قطن / cotton".to_string());
                summary_points.push("prefers cotton".to_string());
            }

            // Extract city / location hints
            for city in &["معادي", "maadi", "زايد", "zayed", "تجمع", "tagamoa", "اسكندرية", "alexandria", "قاهرة", "cairo"] {
                if lower.contains(city) {
                    delta.city = Some(city.to_string());
                    break;
                }
            }

            // Check tool calls for purchases or orders
            if let Some(ref calls) = msg.tool_calls {
                for call in calls {
                    if call.function.name.contains("order") || call.function.name.contains("buy") {
                        if let Ok(args) = serde_json::from_str::<serde_json::Value>(&call.function.arguments) {
                            let sku = args.get("sku").or_else(|| args.get("product_id")).and_then(|v| v.as_str()).unwrap_or("unknown");
                            let name = args.get("item").or_else(|| args.get("name")).and_then(|v| v.as_str()).unwrap_or("item");
                            let price = args.get("price").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let qty = args.get("quantity").and_then(|v| v.as_u64()).unwrap_or(1) as u32;

                            delta.new_purchases.push(PurchaseRecord {
                                sku: sku.to_string(),
                                name: name.to_string(),
                                price,
                                quantity: qty,
                                timestamp: chrono::Utc::now().to_rfc3339(),
                            });
                        }
                    }
                }
            }
        }

        if !summary_points.is_empty() {
            delta.distilled_summary = format!("Customer inquiries indicated: {}.", summary_points.join(", "));
        }

        delta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_customer_dossier_lifecycle_and_anchor() {
        let mut dossier = CustomerDossier::new("01012345678");
        dossier.name = Some("Ahmed Mahmoud".to_string());
        dossier.city = Some("Maadi".to_string());
        dossier.preferences.insert("size".to_string(), "L".to_string());
        dossier.preferences.insert("fabric".to_string(), "100% Egyptian Cotton".to_string());

        dossier.purchases.push(PurchaseRecord {
            sku: "cX23".to_string(),
            name: "Premium Black T-Shirt".to_string(),
            price: 400.0,
            quantity: 2,
            timestamp: "2026-11-04T08:00:00Z".to_string(),
        });
        dossier.distilled_summary = "VIP repeat buyer, prefers dark luxury fabrics.".to_string();

        let anchor = dossier.format_epistemic_anchor();
        assert!(anchor.contains("Ahmed Mahmoud"));
        assert!(anchor.contains("01012345678"));
        assert!(anchor.contains("Maadi"));
        assert!(anchor.contains("size=L"));
        assert!(anchor.contains("cX23"));
        assert!(anchor.contains("VIP repeat buyer"));

        // Verify token compactness: anchor length should be well under 300 characters (~40 tokens)
        assert!(anchor.len() < 350, "Epistemic anchor must remain compact to preserve KV-cache");
    }

    #[test]
    fn test_dossier_persistence_atomic() {
        let temp_dir = tempfile::tempdir().unwrap();
        let base_path = temp_dir.path();

        let mut dossier = CustomerDossier::new("01234567890");
        dossier.name = Some("Sara".to_string());
        dossier.save_to_disk(base_path).expect("Save must succeed");

        let loaded = CustomerDossier::load_or_create(base_path, "01234567890").expect("Load must succeed");
        assert_eq!(loaded.name, Some("Sara".to_string()));
        assert_eq!(loaded.visit_count, 2);
    }

    #[test]
    fn test_distillation_delta_application() {
        let mut dossier = CustomerDossier::new("01112223334");
        
        let mut delta = DistillationDelta::default();
        delta.name = Some("Karim".to_string());
        delta.city = Some("Zayed".to_string());
        delta.preferences.insert("size".to_string(), "XL".to_string());
        delta.distilled_summary = "Inquired about summer shorts.".to_string();
        delta.unresolved_queries.push("Waiting for navy blue restock".to_string());

        dossier.apply_delta(delta);

        assert_eq!(dossier.name, Some("Karim".to_string()));
        assert_eq!(dossier.city, Some("Zayed".to_string()));
        assert_eq!(dossier.preferences.get("size").unwrap(), "XL");
        assert_eq!(dossier.unresolved_queries.len(), 1);
        assert_eq!(dossier.unresolved_queries[0], "Waiting for navy blue restock");
    }

    #[test]
    fn test_parse_distillation_response_valid() {
        let response_text = r#"
        Here is the distilled analysis:
        {
          "name": "Mahmoud",
          "city": "Alexandria",
          "preferences": {
            "size": "XXL",
            "material": "linen"
          },
          "new_purchases": [
            {
              "sku": "cX23",
              "name": "Linen Shirt",
              "price": 400.0,
              "quantity": 1,
              "timestamp": "2026-11-04T09:00:00Z"
            }
          ],
          "unresolved_queries": [],
          "distilled_summary": "Purchased cX23 linen shirt, likes breathable garments."
        }
        "#;

        let delta = PlannerDistiller::parse_distillation_response(response_text).unwrap();
        assert_eq!(delta.name, Some("Mahmoud".to_string()));
        assert_eq!(delta.city, Some("Alexandria".to_string()));
        assert_eq!(delta.preferences.get("size").unwrap(), "XXL");
        assert_eq!(delta.new_purchases.len(), 1);
        assert_eq!(delta.new_purchases[0].sku, "cX23");
    }

    #[test]
    fn test_heuristic_distill() {
        let messages = vec![
            ChatMessage::new("user", "السلام عليكم، مقاس L متوفر من التيشيرت القطن؟"),
            ChatMessage::new("assistant", "وعليكم السلام، أيوه متاح قطن 100%."),
            ChatMessage::new("user", "تمام انا ساكن في المعادي ابعتلي عليه."),
        ];

        let delta = PlannerDistiller::heuristic_distill(&messages);
        assert_eq!(delta.preferences.get("size").unwrap(), "L");
        assert_eq!(delta.preferences.get("fabric").unwrap(), "قطن / cotton");
        assert_eq!(delta.city.unwrap(), "معادي");
        assert!(delta.distilled_summary.contains("size L"));
    }
}
