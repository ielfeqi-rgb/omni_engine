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
use std::process::Command;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductRecord {
    pub sku: String,
    pub name: String,
    pub price: f64,
    pub currency: String,
    pub material: String,
    pub sizes: String,
    pub colors: String,
    pub stock_quantity: i32,
    pub category: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRecord {
    pub order_id: i64,
    pub customer_phone: String,
    pub customer_name: String,
    pub sku: String,
    pub item_name: String,
    pub quantity: i32,
    pub unit_price: f64,
    pub total_price: f64,
    pub address: String,
    pub order_date: String,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteToolRequest {
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

pub struct MarketDb;

impl MarketDb {
    pub fn list_products() -> Result<Vec<ProductRecord>, String> {
        let script = r#"
import sqlite3, json, sys
conn = sqlite3.connect('data/market.db')
conn.row_factory = sqlite3.Row
c = conn.cursor()
c.execute('SELECT * FROM products ORDER BY price ASC')
rows = [dict(r) for r in c.fetchall()]
print(json.dumps(rows, ensure_ascii=False))
conn.close()
"#;
        let output = Command::new("python3")
            .arg("-c")
            .arg(script)
            .output()
            .map_err(|e| format!("Failed to spawn python sqlite runner: {}", e))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("SQLite product query failed: {}", err));
        }

        let out_str = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str::<Vec<ProductRecord>>(out_str.trim())
            .map_err(|e| format!("Failed to parse products JSON: {}", e))
    }

    pub fn list_orders() -> Result<Vec<OrderRecord>, String> {
        let script = r#"
import sqlite3, json, sys
conn = sqlite3.connect('data/market.db')
conn.row_factory = sqlite3.Row
c = conn.cursor()
c.execute('SELECT * FROM orders ORDER BY order_id DESC')
rows = [dict(r) for r in c.fetchall()]
print(json.dumps(rows, ensure_ascii=False))
conn.close()
"#;
        let output = Command::new("python3")
            .arg("-c")
            .arg(script)
            .output()
            .map_err(|e| format!("Failed to spawn python sqlite runner: {}", e))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("SQLite order query failed: {}", err));
        }

        let out_str = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str::<Vec<OrderRecord>>(out_str.trim())
            .map_err(|e| format!("Failed to parse orders JSON: {}", e))
    }

    pub fn execute_tool(tool_name: &str, args: &serde_json::Value) -> Result<serde_json::Value, String> {
        info!("Executing Market DB tool: {} with args: {:?}", tool_name, args);
        match tool_name {
            "search_products" | "find_products" => {
                let query = args.get("query")
                    .or_else(|| args.get("keyword"))
                    .or_else(|| args.get("category"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let clean_q = query.trim().replace('\'', "\\'");
                let script = format!(
                    r#"
import sqlite3, json
conn = sqlite3.connect('data/market.db')
conn.row_factory = sqlite3.Row
c = conn.cursor()
raw_q = '{}'
pattern = f"%{{raw_q}}%"
c.execute('''
    SELECT * FROM products 
    WHERE name LIKE ? OR description LIKE ? OR material LIKE ? OR category LIKE ? OR sku LIKE ?
''', (pattern, pattern, pattern, pattern, pattern))
rows = [dict(r) for r in c.fetchall()]

if not rows:
    syn_groups = [
        {{'بنطالون', 'بنطلون', 'بنطال'}},
        {{'تيشيرت', 'تيشرت'}},
        {{'جاكيت', 'جاكت', 'جاكيته', 'وتربروف', 'ووتربروف', 'مطر'}},
        {{'قميص', 'بولو'}},
    ]
    tokens = set()
    for word in raw_q.replace(',', ' ').replace('؟', ' ').replace('?', ' ').split():
        w = word.strip()
        if len(w) >= 3:
            tokens.add(w)
            for grp in syn_groups:
                if w in grp or any(g in w for g in grp):
                    tokens.update(grp)

    clauses = []
    params = []
    for t in tokens:
        clauses.append('(name LIKE ? OR description LIKE ? OR material LIKE ? OR category LIKE ? OR sku LIKE ?)')
        p = f"%{{t}}%"
        params.extend([p, p, p, p, p])
    if clauses:
        sql = 'SELECT * FROM products WHERE ' + ' OR '.join(clauses)
        c.execute(sql, params)
        seen = set()
        for r in c.fetchall():
            d = dict(r)
            if d['sku'] not in seen:
                seen.add(d['sku'])
                rows.append(d)

print(json.dumps({{"found": len(rows) > 0, "count": len(rows), "products": rows}}, ensure_ascii=False))
conn.close()
"#,
                    clean_q
                );

                let output = Command::new("python3")
                    .arg("-c")
                    .arg(script)
                    .output()
                    .map_err(|e| format!("Search tool failed: {}", e))?;

                let out_str = String::from_utf8_lossy(&output.stdout);
                serde_json::from_str::<serde_json::Value>(out_str.trim())
                    .map_err(|e| format!("Failed to parse search output JSON: {}", e))
            }
            "get_product_price" | "check_stock" | "get_product_details" => {
                let sku = args.get("sku")
                    .or_else(|| args.get("product_id"))
                    .or_else(|| args.get("code"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let clean_sku = sku.trim();
                let script = format!(
                    r#"
import sqlite3, json
conn = sqlite3.connect('data/market.db')
conn.row_factory = sqlite3.Row
c = conn.cursor()
c.execute('SELECT * FROM products WHERE sku = ?', ("{}",))
row = c.fetchone()
if row:
    print(json.dumps({{"found": True, "product": dict(row)}}, ensure_ascii=False))
else:
    c.execute('SELECT * FROM products WHERE LOWER(sku) = LOWER(?)', ("{}",))
    row = c.fetchone()
    if row:
        print(json.dumps({{"found": True, "note": "case_variant", "product": dict(row)}}, ensure_ascii=False))
    else:
        print(json.dumps({{"found": False, "error": "Product with SKU {} not found"}}, ensure_ascii=False))
conn.close()
"#,
                    clean_sku, clean_sku, clean_sku
                );

                let output = Command::new("python3")
                    .arg("-c")
                    .arg(script)
                    .output()
                    .map_err(|e| format!("Tool execution failed: {}", e))?;

                let out_str = String::from_utf8_lossy(&output.stdout);
                serde_json::from_str::<serde_json::Value>(out_str.trim())
                    .map_err(|e| format!("Failed to parse tool output JSON: {}", e))
            }
            "create_order" | "place_order" => {
                let sku = args.get("sku").and_then(|v| v.as_str()).unwrap_or("unknown");
                let name = args.get("customer_name").and_then(|v| v.as_str()).unwrap_or("Customer");
                let phone = args.get("phone").and_then(|v| v.as_str()).unwrap_or("01000000000");
                let address = args.get("address").and_then(|v| v.as_str()).unwrap_or("Cairo");
                let qty = args.get("quantity").and_then(|v| v.as_i64()).unwrap_or(1);

                let script = format!(
                    r#"
import sqlite3, json, datetime
conn = sqlite3.connect('data/market.db')
conn.row_factory = sqlite3.Row
c = conn.cursor()
c.execute('SELECT * FROM products WHERE sku = ?', ("{}",))
row = c.fetchone()
if not row:
    c.execute('SELECT * FROM products WHERE LOWER(sku) = LOWER(?)', ("{}",))
    row = c.fetchone()

if row:
    prod = dict(row)
    price = prod['price']
    item_name = prod['name']
    total = price * {}
    now = datetime.datetime.now().strftime('%Y-%m-%d %H:%M:%S')
    c.execute('''
        INSERT INTO orders (customer_phone, customer_name, sku, item_name, quantity, unit_price, total_price, address, order_date)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
    ''', ("{}", "{}", prod['sku'], item_name, {}, price, total, "{}", now))
    order_id = c.lastrowid
    c.execute('UPDATE products SET stock_quantity = stock_quantity - ? WHERE sku = ?', ({}, prod['sku']))
    conn.commit()
    print(json.dumps({{
        "status": "success",
        "order_id": order_id,
        "item": item_name,
        "sku": prod['sku'],
        "quantity": {},
        "total_price": total,
        "currency": "EGP",
        "address": "{}"
    }}, ensure_ascii=False))
else:
    print(json.dumps({{"status": "error", "message": "Product SKU not found"}}, ensure_ascii=False))
conn.close()
"#,
                    sku, sku, qty, phone, name, qty, address, qty, qty, address
                );

                let output = Command::new("python3")
                    .arg("-c")
                    .arg(script)
                    .output()
                    .map_err(|e| format!("Order creation failed: {}", e))?;

                let out_str = String::from_utf8_lossy(&output.stdout);
                serde_json::from_str::<serde_json::Value>(out_str.trim())
                    .map_err(|e| format!("Failed to parse order output JSON: {}", e))
            }
            _ => Err(format!("Unrecognized tool name '{}'", tool_name))
        }
    }
}
