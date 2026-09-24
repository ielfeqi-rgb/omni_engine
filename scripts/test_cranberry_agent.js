const { spawn } = require("child_process");

async function main() {
    console.log("==========================================================================");
    console.log("   OMNI AUTONOMOUS AGENT - REAL INQUIRY: CRANBERRY ALLERGY SYMPTOMS       ");
    console.log("   Subject: Clinical identification of Cranberry Allergy from real source ");
    console.log("   Observer Mode: Antigravity | Model: Qwen-0.5B Local                    ");
    console.log("==========================================================================\n");

    const userQuestion = "What are the specific symptoms of cranberry allergy according to an authoritative medical source?";
    console.log(`[USER QUESTION]: "${userQuestion}"\n`);

    const chrome = spawn("google-chrome", [
        "--headless=new",
        "--ozone-platform=headless",
        "--no-sandbox",
        "--disable-gpu",
        "--remote-debugging-port=9222",
        "--user-data-dir=/tmp/omni_cranberry_run",
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

    // Target Medical Article
    const targetMedicalUrl = "https://www.wyndly.com/blogs/learn/cranberry-allergy";
    console.log(`=== [STEP 1] Navigating to Medical Source: ${targetMedicalUrl} ===`);

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

    await send("Page.navigate", { url: targetMedicalUrl });
    await Promise.race([ navPromise, new Promise(r => setTimeout(r, 4000)) ]);
    await new Promise(r => setTimeout(r, 2000));

    console.log("=== [STEP 2] Projecting Page and Locating 'Symptoms' Section ===");
    
    // Projector that specifically scans the DOM for headings related to Symptoms
    const symptomScannerScript = `
        (() => {
            const headings = Array.from(document.querySelectorAll("h1, h2, h3, h4"));
            let symptomSections = [];

            for (const h of headings) {
                const txt = h.innerText.trim();
                if (txt.toLowerCase().includes("symptom") || txt.toLowerCase().includes("reaction")) {
                    let sectionBody = [];
                    let sibling = h.nextElementSibling;
                    let count = 0;
                    while (sibling && count < 3) {
                        const tag = sibling.tagName.toLowerCase();
                        if (tag === "p" || tag === "ul" || tag === "div") {
                            const pText = sibling.innerText.trim();
                            if (pText.length > 20) sectionBody.push(pText);
                        }
                        sibling = sibling.nextElementSibling;
                        count++;
                    }
                    symptomSections.push({
                        heading: txt,
                        body: sectionBody.join("\\n")
                    });
                }
            }

            return JSON.stringify({
                pageTitle: document.title,
                url: window.location.href,
                sections: symptomSections.slice(0, 3)
            });
        })()
    `;

    const evalRes = await send("Runtime.evaluate", { expression: symptomScannerScript });
    const parsedData = JSON.parse(evalRes.result.value);

    console.log(`Source Page Title: ${parsedData.pageTitle}`);
    console.log(`Source Page URL:   ${parsedData.url}`);
    console.log(`Located ${parsedData.sections.length} medical symptom sections on the page.\n`);

    console.log("--- [ EXTRACTED SYMPTOM SECTIONS FROM PAGE ] ---");
    let combinedSymptomsText = "";
    for (const sec of parsedData.sections) {
        console.log(`### ${sec.heading}`);
        console.log(`${sec.body}\n`);
        combinedSymptomsText += `[${sec.heading}]\n${sec.body}\n\n`;
    }
    console.log("------------------------------------------------\n");

    // Ask Qwen-0.5B to formulate the clinical summary
    console.log("=== [STEP 3] Awaiting Qwen-0.5B Clinical Synthesis & Citation ===");

    const clinicalPrompt = `User Question: "${userQuestion}"

Medical Source Document:
Title: ${parsedData.pageTitle}
URL: ${parsedData.url}

Extracted Medical Facts:
${combinedSymptomsText}

Instructions for Assistant:
1. Identify and list all the specific symptoms of cranberry allergy mentioned in the text (group by mild/moderate and severe).
2. Note the route of exposure mentioned in the text.
3. Provide the verified source URL.

Your Clinical Response:`;

    const qwenRes = await fetch("http://127.0.0.1:8081/v1/chat/completions", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
            messages: [
                { role: "system", content: "You are a professional medical researcher. Answer the inquiry with high accuracy based strictly on the provided medical text." },
                { role: "user", content: clinicalPrompt }
            ],
            temperature: 0.1,
            max_tokens: 300
        })
    });

    const qwenData = await qwenRes.json();
    const finalReport = qwenData.choices[0].message.content.trim();

    console.log("==================== [ FINAL VERIFIED MEDICAL REPORT ] ====================");
    console.log(finalReport);
    console.log("===========================================================================");

    ws.close();
    chrome.kill();
}

main().catch(console.error);
