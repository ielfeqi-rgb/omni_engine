const { spawn } = require("child_process");

async function main() {
    console.log("=== [1] Launching Chrome Headless for SauceDemo ===");
    const chrome = spawn("google-chrome", [
        "--headless=new",
        "--ozone-platform=headless",
        "--no-sandbox",
        "--disable-gpu",
        "--remote-debugging-port=9222",
        "--user-data-dir=/tmp/omni_sauce_test",
        "about:blank"
    ]);

    await new Promise(r => setTimeout(r, 1200));

    const newTabRes = await fetch("http://127.0.0.1:9222/json/new", { method: "PUT" });
    const tabData = await newTabRes.json();
    const ws = new WebSocket(tabData.webSocketDebuggerUrl);
    await new Promise(r => ws.onopen = r);

    let msgId = 1;
    function send(method, params = {}) {
        return new Promise((resolve) => {
            const id = msgId++;
            const handler = (event) => {
                const data = JSON.parse(event.data);
                if (data.id === id) {
                    ws.removeEventListener("message", handler);
                    resolve(data.result);
                }
            };
            ws.addEventListener("message", handler);
            ws.send(JSON.stringify({ id, method, params }));
        });
    }

    await send("Page.enable");
    await send("Runtime.enable");

    console.log("=== [2] Navigating to https://www.saucedemo.com/ ===");
    const navPromise = new Promise(resolve => {
        const handler = (event) => {
            const data = JSON.parse(event.data);
            if (data.method === "Page.loadEventFired") {
                ws.removeEventListener("message", handler);
                resolve();
            }
        };
        ws.addEventListener("message", handler);
    });

    await send("Page.navigate", { url: "https://www.saucedemo.com/" });
    await navPromise;
    await new Promise(r => setTimeout(r, 1000));

    // The ASCII Projector
    const projectorCode = `
        (() => {
            const cols = 85;
            const rows = 24;
            let grid = Array.from({ length: rows }, () => Array(cols).fill(" "));
            const vpW = window.innerWidth || 1000;
            const vpH = window.innerHeight || 800;
            const charW = vpW / cols;
            const charH = vpH / rows;

            function writeGrid(c, r, text) {
                if (r < 0 || r >= rows) return;
                for (let i = 0; i < text.length; i++) {
                    if (c + i >= 0 && c + i < cols) grid[r][c + i] = text[i];
                }
            }

            // Text elements
            document.querySelectorAll("h1, h2, h3, h4, div.login_logo, div.error-message-container, span.title, div.inventory_item_name").forEach(el => {
                const rect = el.getBoundingClientRect();
                if (rect.width > 0 && rect.height > 0) {
                    const col = Math.floor(rect.left / charW);
                    const row = Math.floor(rect.top / charH);
                    writeGrid(col, row, el.innerText.trim().slice(0, 45));
                }
            });

            // Interactive Elements
            let idx = 1;
            let map = {};
            document.querySelectorAll("input, button, a").forEach(el => {
                const rect = el.getBoundingClientRect();
                if (rect.width > 0 && rect.height > 0) {
                    const col = Math.floor(rect.left / charW);
                    const row = Math.floor(rect.top / charH);
                    const tag = el.tagName.toLowerCase();
                    const txt = (el.value || el.placeholder || el.innerText || "").trim();
                    const id = idx++;
                    const isBtn = tag === "button" || (tag === "input" && (el.type === "submit" || el.type === "button"));
                    map[id] = {
                        id,
                        tag: isBtn ? "button" : tag,
                        type: el.type || "text",
                        placeholder: el.placeholder || "",
                        value: el.value || "",
                        selector: el.id ? "#" + el.id : (el.name ? "[name=\x27" + el.name + "\x27]" : tag)
                    };
                    writeGrid(col, row, "[" + id + ":" + (txt ? txt.slice(0, 15) : (isBtn ? "Button" : tag)) + "]");
                }
            });

            const canvas = grid.map(r => r.join("")).join("\\n");
            return JSON.stringify({ canvas, map, url: window.location.href });
        })()
    `;

    console.log("=== [3] Extracting 2D Terminal ASCII Canvas ===");
    let evalRes = await send("Runtime.evaluate", { expression: projectorCode });
    let pageState = JSON.parse(evalRes.result.value);

    console.log("\n--- [ INITIAL TERMINAL VIEW ] ---");
    console.log(pageState.canvas);
    console.log("---------------------------------");
    console.log("Indexed Controls:", JSON.stringify(pageState.map, null, 2));

    const elementsList = Object.values(pageState.map)
        .map(e => `[${e.id}] ${e.tag} (placeholder="${e.placeholder}", value="${e.value}")`)
        .join("\n");

    const promptText = `Current URL: ${pageState.url}
Terminal Screen:
${pageState.canvas}

Interactive Elements:
${elementsList}

Task: Fill in login credentials and submit.
- Username field [1]: "standard_user"
- Password field [2]: "secret_sauce"
- Submit button [3]: click it

Example format:
type(1, "username_example")
type(2, "password_example")
click(3)

Your actions:`;

    console.log("\n=== [4] Calling Qwen-0.5B Local Brain ===");
    const qwenRes = await fetch("http://127.0.0.1:8081/v1/chat/completions", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
            messages: [
                { role: "system", content: "You are an autonomous web terminal agent. Output only actions like type(1, \"text\") and click(3)." },
                { role: "user", content: promptText }
            ],
            temperature: 0.1,
            max_tokens: 100
        })
    });

    const qwenData = await qwenRes.json();
    const aiResponse = qwenData.choices[0].message.content;
    console.log("Qwen-0.5B Output:\n" + aiResponse);

    console.log("\n=== [5] Executing AI Actions in Chrome DOM ===");
    const lines = aiResponse.split("\n");
    for (const line of lines) {
        const typeMatch = line.match(/type\s*\(\s*(\d+)\s*,\s*["'](.*?)["']\s*\)/);
        const clickMatch = line.match(/click\s*\(\s*(\d+)\s*\)/);

        if (typeMatch) {
            const id = parseInt(typeMatch[1]);
            const text = typeMatch[2];
            const elInfo = pageState.map[id];
            if (elInfo) {
                if (elInfo.tag === "button" || elInfo.type === "submit") {
                    console.log(`> Smart Hand: Element ${id} is a button/submit. Interpreting as click(${id}).`);
                    await send("Runtime.evaluate", {
                        expression: `document.querySelector("${elInfo.selector}").click();`
                    });
                } else {
                    console.log(`> Executing: Typing "${text}" into ${elInfo.selector}`);
                    await send("Runtime.evaluate", {
                        expression: `
                            (() => {
                                const el = document.querySelector("${elInfo.selector}");
                                if (el) {
                                    el.focus();
                                    const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value")?.set;
                                    if (setter) {
                                        setter.call(el, "${text}");
                                    } else {
                                        el.value = "${text}";
                                    }
                                    el.dispatchEvent(new Event("input", { bubbles: true }));
                                    el.dispatchEvent(new Event("change", { bubbles: true }));
                                }
                            })()
                        `
                    });
                }
            }
        } else if (clickMatch) {
            const id = parseInt(clickMatch[1]);
            const elInfo = pageState.map[id];
            if (elInfo) {
                console.log(`> Executing: Clicking ${elInfo.selector}`);
                await send("Runtime.evaluate", {
                    expression: `
                        (() => {
                            const el = document.querySelector("${elInfo.selector}");
                            if (el) el.click();
                        })()
                    `
                });
            }
        }
    }

    console.log("\nWaiting 2.5 seconds for navigation / login response...");
    await new Promise(r => setTimeout(r, 2500));

    console.log("=== [6] Re-projecting New Page State ===");
    evalRes = await send("Runtime.evaluate", { expression: projectorCode });
    pageState = JSON.parse(evalRes.result.value);

    console.log("\n--- [ NEW TERMINAL VIEW AFTER AI LOGIN ] ---");
    console.log("Current Page URL:", pageState.url);
    console.log(pageState.canvas);
    console.log("--------------------------------------------");

    if (pageState.url.includes("inventory.html")) {
        console.log("\n>>> SUCCESS: Qwen-0.5B successfully logged into SauceDemo!");
    } else {
        console.log("\n>>> Check page state.");
    }

    ws.close();
    chrome.kill();
}

main().catch(console.error);
