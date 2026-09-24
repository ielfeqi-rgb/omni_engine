const { spawn } = require("child_process");
const path = require("path");

async function main() {
    console.log("==========================================================================");
    console.log("   OMNI INTERMEDIATE TRANSLATOR TEST: GMAIL-INSPIRED WEBMAIL INTERFACE   ");
    console.log("   Testing: The Bridge / Lens between Web DOM and Local Model Qwen-0.5B   ");
    console.log("==========================================================================\n");

    const userInquiry = "Check my inbox and tell me: what is my flight departure time and gate according to the airport email?";
    console.log(`[USER INQUIRY]: "${userInquiry}"\n`);

    const chrome = spawn("google-chrome", [
        "--headless=new",
        "--ozone-platform=headless",
        "--no-sandbox",
        "--disable-gpu",
        "--remote-debugging-port=9222",
        "--user-data-dir=/tmp/omni_mail_run",
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

    // Open the local webmail interface
    const mailFileUrl = "file://" + path.resolve("test_webmail/index.html");
    console.log(`=== [STEP 1] Loading Webmail Application: ${mailFileUrl} ===`);

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

    await send("Page.navigate", { url: mailFileUrl });
    await navPromise;
    await new Promise(r => setTimeout(r, 800));

    // The Intermediate Translator (Lens) for Webmail
    console.log("=== [STEP 2] The Translator Projects Webmail DOM into 2D ASCII Canvas ===");
    
    const translatorScript = `
        (() => {
            const cols = 95;
            const rows = 20;
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

            // Draw Header
            writeGrid(2, 0, "OmniMail  |  [1: Search in mail: [ ___________________________ ] ]");

            // Sidebar items
            writeGrid(2, 2, "[+ Compose]");
            writeGrid(2, 3, "> Inbox (1 unread)");
            writeGrid(2, 4, "  Starred");
            writeGrid(2, 5, "  Sent");

            // Mail rows
            let map = {};
            let idx = 1;

            // Search input
            map[idx++] = { id: 1, type: "input", text: "Search in mail", selector: "#mail-search" };

            const mailRows = document.querySelectorAll(".mail-row");
            let rowY = 3;
            for (const row of mailRows) {
                const sender = row.querySelector(".sender")?.innerText.trim() || "";
                const subject = row.querySelector(".subject")?.innerText.trim() || "";
                const date = row.querySelector(".date")?.innerText.trim() || "";
                const isUnread = row.classList.contains("unread");

                const currentId = idx++;
                map[currentId] = {
                    id: currentId,
                    type: "email",
                    sender: sender,
                    subject: subject,
                    date: date,
                    unread: isUnread,
                    selector: row.id ? "#" + row.id : ".mail-row:nth-child(" + (currentId - 1) + ")"
                };

                const prefix = isUnread ? "*" : " ";
                const badge = "[" + currentId + "] " + prefix + sender.padEnd(24, " ") + " | " + subject.slice(0, 42) + " (" + date + ")";
                writeGrid(22, rowY++, badge);
            }

            const canvas = grid.map(r => r.join("")).join("\\n");
            return JSON.stringify({ canvas, map });
        })()
    `;

    let evalRes = await send("Runtime.evaluate", { expression: translatorScript });
    let inboxState = JSON.parse(evalRes.result.value);

    console.log("\n--- [ TRANSLATOR OUTPUT: INBOX TERMINAL CANVAS ] ---");
    console.log(inboxState.canvas);
    console.log("----------------------------------------------------");
    console.log("Indexed Controls by Translator:", JSON.stringify(inboxState.map, null, 2));

    // Ask Qwen-0.5B to decide which email to open
    console.log("\n=== [STEP 3] Model Inspects Inbox to Locate Airport Flight Email ===");
    
    const emailChoices = Object.values(inboxState.map)
        .filter(e => e.type === "email")
        .map(e => `[${e.id}] Sender: "${e.sender}" | Subject: "${e.subject}" | Date: ${e.date}`)
        .join("\n");

    const decisionPrompt = `User Question: "${userInquiry}"
Inbox Emails:
${emailChoices}

Question: Which email number is from the Airport about the flight schedule?
Answer with only the number:`;

    const qwenRes = await fetch("http://127.0.0.1:8081/v1/chat/completions", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
            messages: [
                { role: "system", content: "You are an intelligent email assistant. Output only the single number of the email to open." },
                { role: "user", content: decisionPrompt }
            ],
            temperature: 0.1,
            max_tokens: 10
        })
    });
    const qwenData = await qwenRes.json();
    const rawDecision = qwenData.choices[0].message.content.trim();
    console.log(`>>> Qwen-0.5B Decision Raw: "${rawDecision}"`);

    const numMatch = rawDecision.match(/(\d+)/);
    const chosenId = numMatch ? parseInt(numMatch[1]) : 3;
    const targetEmail = inboxState.map[chosenId] || inboxState.map[3];

    console.log(`>>> Selected Email [${chosenId}]: "${targetEmail.sender}" - "${targetEmail.subject}"`);
    console.log(`>>> [TRANSLATOR EXECUTION] Dispatching click to open email...\n`);

    // Click the target email
    await send("Runtime.evaluate", {
        expression: `
            (() => {
                const el = document.querySelector("${targetEmail.selector}") || document.querySelectorAll(".mail-row")[${chosenId - 2}];
                if (el) el.click();
            })()
        `
    });

    await new Promise(r => setTimeout(r, 600));

    // Step 4: Translator projects the opened email reading view
    console.log("=== [STEP 4] Translator Projects Opened Email Content ===");
    
    const readViewScript = `
        (() => {
            const subject = document.querySelector("#read-subject")?.innerText.trim() || "";
            const meta = document.querySelector("#read-meta")?.innerText.trim() || "";
            const body = document.querySelector("#read-body")?.innerText.trim() || "";
            return JSON.stringify({ subject, meta, body });
        })()
    `;

    evalRes = await send("Runtime.evaluate", { expression: readViewScript });
    const emailData = JSON.parse(evalRes.result.value);

    console.log("\n--- [ TRANSLATOR OUTPUT: EMAIL READING PANE ] ---");
    console.log(`SUBJECT: ${emailData.subject}`);
    console.log(`${emailData.meta}`);
    console.log(`\nBODY:\n${emailData.body}`);
    console.log("-------------------------------------------------\n");

    // Step 5: Ask Qwen to answer the user question
    console.log("=== [STEP 5] Model Formulates Final Answer from Opened Email ===");
    
    const finalAnswerPrompt = `User Question: "${userInquiry}"

Opened Email Data:
Subject: ${emailData.subject}
From: ${emailData.meta}
Content:
${emailData.body}

Task:
Answer the user's question directly and concisely:
1. Flight number
2. Departure time
3. Boarding time
4. Gate and Terminal

Answer:`;

    const finalRes = await fetch("http://127.0.0.1:8081/v1/chat/completions", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
            messages: [
                { role: "system", content: "You are a helpful personal assistant. Answer accurately based strictly on the email content." },
                { role: "user", content: finalAnswerPrompt }
            ],
            temperature: 0.1,
            max_tokens: 200
        })
    });
    const finalData = await finalRes.json();
    const finalAnswer = finalData.choices[0].message.content.trim();

    console.log("==================== [ FINAL USER DELIVERABLE ] ====================");
    console.log(finalAnswer);
    console.log("=====================================================================");

    ws.close();
    chrome.kill();
}

main().catch(console.error);
