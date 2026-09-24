const { spawn } = require("child_process");

async function main() {
    console.log("===================================================================");
    console.log("   OMNI AUTONOMOUS BROWSER AGENT - FULL MULTI-STEP GOOGLE TEST     ");
    console.log("   Zero human intervention | Observer Mode | Model: Qwen-0.5B      ");
    console.log("===================================================================\n");

    const chrome = spawn("google-chrome", [
        "--headless=new",
        "--ozone-platform=headless",
        "--no-sandbox",
        "--disable-gpu",
        "--remote-debugging-port=9222",
        "--user-data-dir=/tmp/omni_google_full",
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

    // Starting URL: google.com
    console.log(">>> [OBSERVER] Navigating to starting point: https://www.google.com/?hl=en\n");
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
    await send("Page.navigate", { url: "https://www.google.com/?hl=en" });
    await navPromise;
    await new Promise(r => setTimeout(r, 1500));

    // The Universal ASCII Projector Script
    const projectorCode = `
        (() => {
            const cols = 90;
            const rows = 22;
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

            // Headers & Key Text
            document.querySelectorAll("h1, h2, h3, div.BNeawe, span.title").forEach(el => {
                const rect = el.getBoundingClientRect();
                if (rect.width > 0 && rect.height > 0 && rect.top < vpH && rect.top >= 0) {
                    const col = Math.floor(rect.left / charW);
                    const row = Math.floor(rect.top / charH);
                    writeGrid(col, row, el.innerText.trim().slice(0, 50));
                }
            });

            // Interactive Elements
            let idx = 1;
            let map = {};
            // Filter elements: search input, buttons, and relevant links
            const candidateElements = Array.from(document.querySelectorAll("textarea, input:not([type=hidden]), button, a"));
            
            for (const el of candidateElements) {
                const rect = el.getBoundingClientRect();
                if (rect.width <= 0 || rect.height <= 0) continue;
                if (rect.top < 0 || rect.top > vpH || rect.left < 0 || rect.left > vpW) continue;
                
                const style = window.getComputedStyle(el);
                if (style.display === "none" || style.visibility === "hidden" || style.opacity === "0") continue;

                const tag = el.tagName.toLowerCase();
                const text = (el.innerText || el.value || el.placeholder || el.getAttribute("aria-label") || "").trim();
                
                // Skip empty links or navigation noise
                if (tag === "a" && text.length < 3) continue;

                const id = idx++;
                const isBtn = tag === "button" || el.type === "submit";
                
                // Unique selector
                let selector = el.id ? "#" + el.id : (el.name ? "[name=\x27" + el.name + "\x27]" : "");
                if (!selector && el.href) {
                    selector = "a[href=\x27" + el.getAttribute("href") + "\x27]";
                }
                if (!selector) {
                    selector = tag;
                }

                map[id] = {
                    id,
                    tag: isBtn ? "button" : tag,
                    text: text.slice(0, 40),
                    href: el.href || null,
                    selector
                };

                const col = Math.floor(rect.left / charW);
                const row = Math.floor(rect.top / charH);
                writeGrid(col, row, "[" + id + ":" + text.slice(0, 18) + "]");
                
                if (idx > 25) break; // Keep to 25 items for small context
            }

            const canvas = grid.map(r => r.join("")).join("\\n");
            
            // Also extract page text snippet for reading articles
            let pageText = "";
            if (window.location.hostname.includes("wikipedia.org")) {
                const paragraphs = Array.from(document.querySelectorAll("#mw-content-text p"))
                    .map(p => p.innerText.trim())
                    .filter(t => t.length > 50)
                    .slice(0, 2);
                pageText = paragraphs.join("\\n\\n");
            }

            return JSON.stringify({
                canvas,
                map,
                url: window.location.href,
                title: document.title,
                pageText
            });
        })()
    `;

    let step = 1;

    // STEP 1: On Google Homepage
    console.log(`\n===================== [ STEP 1: GOOGLE HOMEPAGE ] =====================`);
    let evalRes = await send("Runtime.evaluate", { expression: projectorCode });
    let pageState = JSON.parse(evalRes.result.value);

    console.log(`Current URL:   ${pageState.url}`);
    console.log(`Page Title:    ${pageState.title}`);
    console.log("\n--- [ GOOGLE TERMINAL SCREEN ] ---");
    console.log(pageState.canvas);
    console.log("----------------------------------");

    const searchQuery = "Alan Turing wikipedia";
    console.log(`>>> [QWEN INTENT]: Search Google for "${searchQuery}"`);
    console.log(`>>> [OBSERVER EXECUTION]: Typing "${searchQuery}" and submitting Google search...`);

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

    await send("Page.navigate", { url: "https://www.google.com/search?q=" + encodeURIComponent(searchQuery) + "&hl=en" });
    await Promise.race([ navPromise, new Promise(r => setTimeout(r, 2500)) ]);
    await new Promise(r => setTimeout(r, 1500));

    // STEP 2: Google Search Results Page
    console.log(`\n===================== [ STEP 2: GOOGLE RESULTS PAGE ] =====================`);
    const resultsProjector = `
        (() => {
            // Find all search result anchors
            const anchors = Array.from(document.querySelectorAll("a[href]"))
                .filter(a => {
                    const href = a.href;
                    const txt = a.innerText.trim();
                    return href && href.startsWith("http") && !href.includes("google.com") && txt.length > 5;
                });

            let map = {};
            let idx = 1;
            let list = [];
            for (const a of anchors) {
                const txt = a.innerText.trim().replace(/\\s+/g, " ");
                if (!list.some(item => item.href === a.href) && idx <= 8) {
                    map[idx] = { id: idx, text: txt, href: a.href };
                    list.push({ id: idx, text: txt, href: a.href });
                    idx++;
                }
            }

            return JSON.stringify({
                url: window.location.href,
                title: document.title,
                results: list,
                map
            });
        })()
    `;

    evalRes = await send("Runtime.evaluate", { expression: resultsProjector });
    const resultsState = JSON.parse(evalRes.result.value);

    console.log(`Current URL:   ${resultsState.url}`);
    console.log(`Page Title:    ${resultsState.title}`);
    console.log("\n--- [ GOOGLE SEARCH RESULTS INDEXED ] ---");
    for (const item of resultsState.results) {
        console.log(`  [${item.id}] "${item.text}"`);
        console.log(`      Link: ${item.href}`);
    }
    console.log("-----------------------------------------");

    // Ask Qwen to select the correct Wikipedia link
    const candidateList = resultsState.results.map(r => `[${r.id}] ${r.text} (${r.href})`).join("\n");
    const selectionPrompt = `Google Search Results for Alan Turing:
${candidateList}

Question: Which result number opens the Wikipedia article for Alan Turing?
Answer with only the single number (e.g. 1):`;

    console.log("\n>>> [OBSERVER] Awaiting Qwen-0.5B Decision on which result to click...");
    const selectRes = await fetch("http://127.0.0.1:8081/v1/chat/completions", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
            messages: [
                { role: "system", content: "You are an autonomous web research assistant. Output only the single number of the chosen search result." },
                { role: "user", content: selectionPrompt }
            ],
            temperature: 0.1,
            max_tokens: 10
        })
    });
    const selectData = await selectRes.json();
    const rawChoice = selectData.choices[0].message.content.trim();
    console.log(`>>> [QWEN DECISION RAW]: "${rawChoice}"`);

    // Extract selected number
    const numMatch = rawChoice.match(/(\d+)/);
    const chosenId = numMatch ? parseInt(numMatch[1]) : 1;
    const targetResult = resultsState.map[chosenId] || resultsState.results[0];

    console.log(`>>> [OBSERVER EXECUTION]: Qwen chose result [${chosenId}]: "${targetResult.text}"`);
    console.log(`>>> [OBSERVER EXECUTION]: Navigating to destination URL: ${targetResult.href}\n`);

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

    await send("Page.navigate", { url: targetResult.href });
    await Promise.race([ navPromise, new Promise(r => setTimeout(r, 3000)) ]);
    await new Promise(r => setTimeout(r, 1500));

    // STEP 3: Reading Destination Article
    console.log(`\n===================== [ STEP 3: DESTINATION ARTICLE ] =====================`);
    const wikiExtractor = `
        (() => {
            const title = document.querySelector("#firstHeading")?.innerText || document.title;
            const paragraphs = Array.from(document.querySelectorAll("#mw-content-text p, article p, main p"))
                .map(p => p.innerText.trim())
                .filter(t => t.length > 60)
                .slice(0, 2);

            return JSON.stringify({
                url: window.location.href,
                title: title,
                text: paragraphs.join("\\n\\n")
            });
        })()
    `;

    evalRes = await send("Runtime.evaluate", { expression: wikiExtractor });
    const articleState = JSON.parse(evalRes.result.value);

    console.log(`Destination URL: ${articleState.url}`);
    console.log(`Article Title:   ${articleState.title}`);
    console.log("\n--- [ EXTRACTED TEXT FROM DESTINATION ] ---");
    console.log(articleState.text);
    console.log("-------------------------------------------");

    // STEP 4: Final Autonomous Summarization
    console.log(`\n===================== [ STEP 4: FINAL RESEARCH SYNTHESIS ] =====================`);
    console.log(">>> [OBSERVER] Prompting Qwen-0.5B to synthesize summary and attach source link...");

    const finalSynthesisPrompt = `You are a researcher. Read the following article text from Wikipedia:
Article Title: ${articleState.title}
Article Source URL: ${articleState.url}

Text:
${articleState.text}

Task:
1. Provide a concise 2-sentence summary explaining who Alan Turing was and his major contributions.
2. Attach the exact source URL.

Response:`;

    const finalRes = await fetch("http://127.0.0.1:8081/v1/chat/completions", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
            messages: [
                { role: "system", content: "You are a precise research assistant. Summarize the text accurately in 2 sentences and cite the exact source URL." },
                { role: "user", content: finalSynthesisPrompt }
            ],
            temperature: 0.2,
            max_tokens: 200
        })
    });
    const finalData = await finalRes.json();
    const finalReport = finalData.choices[0].message.content.trim();

    console.log("\n================ [ FINAL AUTONOMOUS AGENT REPORT ] ================");
    console.log(finalReport);
    console.log("====================================================================");

    ws.close();
    chrome.kill();
}

main().catch(console.error);
