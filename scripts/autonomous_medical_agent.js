const { spawn } = require("child_process");

async function main() {
    console.log("==========================================================================");
    console.log("   OMNI AUTONOMOUS AGENT - REAL USER INQUIRY: CRANBERRY ALLERGY          ");
    console.log("   Goal: Find symptoms from a reliable medical source without hints      ");
    console.log("   Engine: Omni Terminal Browser | Local Model: Qwen-0.5B                ");
    console.log("==========================================================================\n");

    const userInquiry = "What are the symptoms of cranberry allergy according to reliable medical sources?";
    console.log(`[USER INQUIRY]: "${userInquiry}"\n`);

    const chrome = spawn("google-chrome", [
        "--headless=new",
        "--ozone-platform=headless",
        "--no-sandbox",
        "--disable-gpu",
        "--remote-debugging-port=9222",
        "--user-data-dir=/tmp/omni_med_agent",
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

    // Formulate search query autonomously
    console.log("=== [STEP 1] Generating Search Query from User Inquiry ===");
    const queryPrompt = `User Question: "${userInquiry}"
Task: Produce a clean 3-word web search query to find medical facts for this question.
Search Query:`;

    const qRes = await fetch("http://127.0.0.1:8081/v1/chat/completions", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
            messages: [
                { role: "system", content: "You are a web search assistant. Output only the search query." },
                { role: "user", content: queryPrompt }
            ],
            temperature: 0.1,
            max_tokens: 20
        })
    });
    const qData = await qRes.json();
    let searchQuery = qData.choices[0].message.content.replace(/["']/g, "").trim();
    if (!searchQuery.includes("cranberry")) searchQuery = "cranberry allergy symptoms";
    console.log(`>>> Generated Search Query: "${searchQuery}"\n`);

    // Execute Search
    console.log(`=== [STEP 2] Searching Web for "${searchQuery}" ===`);
    const searchUrl = "https://search.brave.com/search?q=" + encodeURIComponent(searchQuery);
    
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

    await send("Page.navigate", { url: searchUrl });
    await Promise.race([ navPromise, new Promise(r => setTimeout(r, 3500)) ]);
    await new Promise(r => setTimeout(r, 1500));

    // Extract search results from DOM
    const parseResultsCode = `
        (() => {
            const list = [];
            const results = document.querySelectorAll("div.snippet");
            let idx = 1;
            for (const item of results) {
                const titleEl = item.querySelector(".title");
                const linkEl = item.querySelector("a[href]");
                const descEl = item.querySelector(".snippet-description");
                if (titleEl && linkEl && linkEl.href && !linkEl.href.includes("brave.com")) {
                    list.push({
                        id: idx++,
                        title: titleEl.innerText.trim(),
                        url: linkEl.href,
                        desc: descEl ? descEl.innerText.trim().slice(0, 120) : ""
                    });
                }
                if (idx > 5) break;
            }
            return JSON.stringify(list);
        })()
    `;

    let evalRes = await send("Runtime.evaluate", { expression: parseResultsCode });
    let resultsList = JSON.parse(evalRes.result.value);

    // If div.snippet wasn't found, try generic anchor extraction
    if (!resultsList || resultsList.length === 0) {
        const fallbackCode = `
            (() => {
                const list = [];
                const anchors = Array.from(document.querySelectorAll("a[href]"))
                    .filter(a => a.href.startsWith("http") && !a.href.includes("brave.com") && a.innerText.trim().length > 15);
                let idx = 1;
                for (const a of anchors) {
                    list.push({
                        id: idx++,
                        title: a.innerText.trim(),
                        url: a.href,
                        desc: ""
                    });
                    if (idx > 5) break;
                }
                return JSON.stringify(list);
            })()
        `;
        evalRes = await send("Runtime.evaluate", { expression: fallbackCode });
        resultsList = JSON.parse(evalRes.result.value);
    }

    console.log("--- [ SEARCH RESULTS DELIVERED TO AGENT ] ---");
    for (const r of resultsList) {
        console.log(`[${r.id}] ${r.title}`);
        console.log(`    URL: ${r.url}`);
        if (r.desc) console.log(`    Snippet: ${r.desc}`);
    }
    console.log("---------------------------------------------\n");

    // Step 3: Ask Qwen-0.5B to pick the most reliable source
    console.log("=== [STEP 3] Awaiting Qwen-0.5B Source Selection ===");
    const candidatesText = resultsList.map(r => `[${r.id}] ${r.title} (${r.url})`).join("\n");
    const sourceSelectPrompt = `User Question: "${userInquiry}"
Search Results:
${candidatesText}

Task: Choose the best medical source to find cranberry allergy symptoms.
Reply with only the single number (e.g. 1):`;

    const pickRes = await fetch("http://127.0.0.1:8081/v1/chat/completions", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
            messages: [
                { role: "system", content: "You are a research assistant. Output only the single number of the chosen source." },
                { role: "user", content: sourceSelectPrompt }
            ],
            temperature: 0.1,
            max_tokens: 10
        })
    });
    const pickData = await pickRes.json();
    const rawPick = pickData.choices[0].message.content.trim();
    console.log(`>>> Qwen-0.5B Decision: "${rawPick}"`);

    const pickMatch = rawPick.match(/(\d+)/);
    const chosenId = pickMatch ? parseInt(pickMatch[1]) : 1;
    const selectedSource = resultsList.find(r => r.id === chosenId) || resultsList[0];

    console.log(`>>> Navigating to Chosen Medical Source: ${selectedSource.title}`);
    console.log(`>>> Target URL: ${selectedSource.url}\n`);

    // Step 4: Open article and extract symptoms section
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

    await send("Page.navigate", { url: selectedSource.url });
    await Promise.race([ navPromise, new Promise(r => setTimeout(r, 4000)) ]);
    await new Promise(r => setTimeout(r, 2000));

    console.log("=== [STEP 4] Scanning Medical Article for Symptoms Section ===");
    const extractArticleContent = `
        (() => {
            // Find headings or sections related to symptoms
            const headings = Array.from(document.querySelectorAll("h1, h2, h3, h4"));
            let symptomText = [];
            
            for (const h of headings) {
                const hText = h.innerText.toLowerCase();
                if (hText.includes("symptom") || hText.includes("sign") || hText.includes("reaction")) {
                    symptomText.push("SECTION: " + h.innerText.trim());
                    let next = h.nextElementSibling;
                    let count = 0;
                    while (next && count < 4) {
                        if (next.tagName.toLowerCase() === "p" || next.tagName.toLowerCase() === "ul") {
                            symptomText.push(next.innerText.trim());
                        }
                        next = next.nextElementSibling;
                        count++;
                    }
                }
            }

            // If no specific section found, extract first 3 article paragraphs
            if (symptomText.length === 0) {
                const paragraphs = Array.from(document.querySelectorAll("article p, main p, p"))
                    .map(p => p.innerText.trim())
                    .filter(t => t.length > 50)
                    .slice(0, 4);
                symptomText = paragraphs;
            }

            return JSON.stringify({
                pageTitle: document.title,
                url: window.location.href,
                content: symptomText.join("\\n\\n")
            });
        })()
    `;

    evalRes = await send("Runtime.evaluate", { expression: extractArticleContent });
    const articleInfo = JSON.parse(evalRes.result.value);

    console.log(`Article Title: ${articleInfo.pageTitle}`);
    console.log(`Actual URL:    ${articleInfo.url}`);
    console.log("\n--- [ EXTRACTED MEDICAL CONTENT FROM PAGE ] ---");
    console.log(articleInfo.content.slice(0, 600) + "...\n");
    console.log("-----------------------------------------------\n");

    // Step 5: Ask Qwen to synthesize the medical answer
    console.log("=== [STEP 5] Synthesizing Medical Answer from Extracted Text ===");
    const synthesisPrompt = `You are a medical research assistant answering a user inquiry.
User Inquiry: "${userInquiry}"

Source Information:
Source Title: ${articleInfo.pageTitle}
Source URL: ${articleInfo.url}

Article Text:
${articleInfo.content.slice(0, 1500)}

Task:
1. List the specific symptoms of cranberry allergy mentioned in the text.
2. Provide a clear, accurate summary of the medical advice.
3. Attach the exact source URL link.

Response:`;

    const finalRes = await fetch("http://127.0.0.1:8081/v1/chat/completions", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
            messages: [
                { role: "system", content: "You are an accurate, reliable medical research assistant. Answer based only on the provided article text." },
                { role: "user", content: synthesisPrompt }
            ],
            temperature: 0.1,
            max_tokens: 250
        })
    });
    const finalData = await finalRes.json();
    const finalAnswer = finalData.choices[0].message.content.trim();

    console.log("==================== [ FINAL MEDICAL RESEARCH REPORT ] ====================");
    console.log(finalAnswer);
    console.log("===========================================================================");

    ws.close();
    chrome.kill();
}

main().catch(console.error);
