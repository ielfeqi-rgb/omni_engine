const { spawn } = require("child_process");

async function main() {
    console.log("===============================================================");
    console.log("   OMNI AUTONOMOUS BROWSER AGENT - WIKIPEDIA RESEARCH TEST    ");
    console.log("   Model: Qwen-0.5B Local (offline CPU)                       ");
    console.log("===============================================================\n");

    console.log("=== [1] Launching Chrome Headless ===");
    const chrome = spawn("google-chrome", [
        "--headless=new",
        "--ozone-platform=headless",
        "--no-sandbox",
        "--disable-gpu",
        "--remote-debugging-port=9222",
        "--user-data-dir=/tmp/omni_wiki_agent",
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

    console.log("=== [2] Navigating to https://en.wikipedia.org/ ===");
    let navPromise = new Promise(resolve => {
        const handler = (event) => {
            const data = JSON.parse(event.data);
            if (data.method === "Page.loadEventFired") {
                ws.removeEventListener("message", handler);
                resolve();
            }
        };
        ws.addEventListener("message", handler);
    });
    await send("Page.navigate", { url: "https://en.wikipedia.org/" });
    await navPromise;
    await new Promise(r => setTimeout(r, 1000));

    // Project homepage ASCII view
    const projectorScript = `
        (() => {
            const cols = 80;
            const rows = 16;
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

            // Visible search input
            let map = {};
            const searchInput = document.querySelector("#searchInput") || document.querySelector("input[type=search]");
            if (searchInput) {
                const rect = searchInput.getBoundingClientRect();
                const col = Math.floor(rect.left / charW);
                const row = Math.floor(rect.top / charH);
                map[1] = {
                    id: 1,
                    tag: "input",
                    placeholder: searchInput.placeholder,
                    selector: "#" + searchInput.id
                };
                writeGrid(col, row, "[1: " + searchInput.placeholder + "]");
            }

            // Header
            writeGrid(2, 1, "WIKIPEDIA - The Free Encyclopedia");

            const canvas = grid.map(r => r.join("")).join("\\n");
            return JSON.stringify({ canvas, map, url: window.location.href });
        })()
    `;

    let evalRes = await send("Runtime.evaluate", { expression: projectorScript });
    let pageState = JSON.parse(evalRes.result.value);

    console.log("\n--- [ STEP 1: WIKIPEDIA TERMINAL SCREEN ] ---");
    console.log(pageState.canvas);
    console.log("----------------------------------------------");
    console.log("Indexed Controls:", JSON.stringify(pageState.map, null, 2));

    // Ask Qwen to perform the search for "Alan Turing"
    const step1Prompt = `Current URL: ${pageState.url}
Terminal Screen:
${pageState.canvas}

Available Controls:
[1] input (placeholder="${pageState.map[1].placeholder}")

Task: Search Wikipedia for "Alan Turing".
Commands format:
search(1, "search query")

Output only the command:`;

    console.log("\n=== [3] Asking Qwen-0.5B to execute search ===");
    let qwenRes = await fetch("http://127.0.0.1:8081/v1/chat/completions", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
            messages: [
                { role: "system", content: "You are an autonomous web terminal agent. Output only search(1, \"query\")." },
                { role: "user", content: step1Prompt }
            ],
            temperature: 0.1,
            max_tokens: 50
        })
    });
    let qwenData = await qwenRes.json();
    let action1 = qwenData.choices[0].message.content.trim();
    console.log("Qwen-0.5B Decision:", action1);

    // Parse search query
    let query = "Alan Turing";
    const searchMatch = action1.match(/search\s*\(\s*\d+\s*,\s*["'](.*?)["']\s*\)/);
    if (searchMatch) {
        query = searchMatch[1];
    }
    console.log(`> Executing Search for query: "${query}"...`);

    // Submit search on Wikipedia
    navPromise = new Promise(resolve => {
        const handler = (event) => {
            const data = JSON.parse(event.data);
            if (data.method === "Page.loadEventFired") {
                ws.removeEventListener("message", handler);
                resolve();
            }
        };
        ws.addEventListener("message", handler);
    });

    await send("Runtime.evaluate", {
        expression: `
            (() => {
                const input = document.querySelector("#searchInput") || document.querySelector("input[type=search]");
                if (input) {
                    input.value = "${query}";
                    const form = input.closest("form");
                    if (form) form.submit();
                    else window.location.href = "https://en.wikipedia.org/wiki/" + encodeURIComponent("${query}");
                }
            })()
        `
    });
    await navPromise;
    await new Promise(r => setTimeout(r, 1500));

    // Extract Article Content
    console.log("\n=== [4] Extracting Article Text & Terminal View ===");
    const articleExtractor = `
        (() => {
            const title = document.querySelector("#firstHeading")?.innerText || document.title;
            const paragraphs = Array.from(document.querySelectorAll("#mw-content-text p"))
                .map(p => p.innerText.trim())
                .filter(txt => txt.length > 80)
                .slice(0, 2);
            
            return JSON.stringify({
                url: window.location.href,
                title: title,
                text: paragraphs.join("\\n\\n")
            });
        })()
    `;

    evalRes = await send("Runtime.evaluate", { expression: articleExtractor });
    const articleData = JSON.parse(evalRes.result.value);

    console.log("Source URL:", articleData.url);
    console.log("Article Title:", articleData.title);
    console.log("\n--- [ ARTICLE EXTRACT (First 2 Paragraphs) ] ---");
    console.log(articleData.text);
    console.log("-------------------------------------------------");

    // Ask Qwen to summarize and cite the source
    console.log("\n=== [5] Asking Qwen-0.5B to Summarize and Attach Source Link ===");
    const summaryPrompt = `You are a researcher. Read the following Wikipedia article text and provide:
1. A concise 2-sentence summary in clear language.
2. The source URL.

Article Title: ${articleData.title}
Article URL: ${articleData.url}
Article Text:
${articleData.text}

Your Response:`;

    qwenRes = await fetch("http://127.0.0.1:8081/v1/chat/completions", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
            messages: [
                { role: "system", content: "You are a precise research assistant. Summarize the provided text in 2 sentences and always output the source URL." },
                { role: "user", content: summaryPrompt }
            ],
            temperature: 0.2,
            max_tokens: 200
        })
    });
    qwenData = await qwenRes.json();
    const finalReport = qwenData.choices[0].message.content.trim();

    console.log("\n================ [ FINAL AI RESEARCH REPORT ] ================");
    console.log(finalReport);
    console.log("==============================================================");

    ws.close();
    chrome.kill();
}

main().catch(console.error);
