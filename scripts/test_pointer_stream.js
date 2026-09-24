const http = require('http');

function queryStream(prompt, options = {}) {
    return new Promise((resolve, reject) => {
        const payload = JSON.stringify({
            prompt: prompt,
            n_predict: options.n_predict || 200,
            temperature: 0.1,
            cache_prompt: true,
            id_slot: 0,
            stop: options.stop || ["<|im_end|>", "<|endoftext|>"]
        });

        const req = http.request({
            hostname: '127.0.0.1',
            port: 8081,
            path: '/completion',
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Content-Length': Buffer.byteLength(payload)
            }
        }, (res) => {
            let body = '';
            res.on('data', chunk => body += chunk);
            res.on('end', () => {
                try {
                    resolve(JSON.parse(body));
                } catch (e) {
                    reject(new Error("Parse error: " + body));
                }
            });
        });

        req.on('error', reject);
        req.write(payload);
        req.end();
    });
}

function parseModelAnswer(text) {
    // Extract either final line or number/string
    const cleaned = text.trim();
    const lines = cleaned.split('\n');
    return lines[lines.length - 1].trim();
}

async function runAutonomousPointerTest() {
    console.log("========================================================================");
    console.log(" AUTONOMOUS CONTINUOUS KV-STREAM POINTER EXPERIMENT (ZERO INTERVENTION)");
    console.log("========================================================================\n");

    const experiments = [
        {
            name: "EXPERIMENT 1: Logical & Simple (Arithmetic Verification)",
            question: "What is 17 + 28? Reply with strictly ONLY the final number, nothing else.",
            evaluator: (ans) => {
                const num = parseInt(ans.replace(/[^0-9]/g, ''), 10);
                if (num === 45) return { ok: true, msg: "Correct" };
                return { ok: false, msg: `Answer unmatched. Expected exact integer sum, got '${ans}'.` };
            }
        },
        {
            name: "EXPERIMENT 2: Guaranteed Failure Trap (Deceptive Sequence & Reversal)",
            // A deceptive trick: "What is the capital of Australia? Answer with one word."
            // Almost all small models reflexively answer "Sydney".
            // Correct answer is "Canberra".
            question: "What is the capital city of Australia? Respond with strictly ONE word only.",
            evaluator: (ans) => {
                const word = ans.replace(/[^a-zA-Z]/g, '').trim().toLowerCase();
                if (word === "canberra") return { ok: true, msg: "Correct" };
                return { ok: false, msg: `Verification Error: '${ans}' is NOT the capital of Australia. Immediate failure.` };
            }
        },
        {
            name: "EXPERIMENT 3: Subtle Challenge (String Constraint & Logic Trick)",
            // Count letters with a trick: "strawberry" contains how many 'r's?
            // Small models often say 2. Correct is 3.
            question: "How many times does the letter 'r' appear in the word 'strawberry'? Answer with strictly the digit only.",
            evaluator: (ans) => {
                const digit = parseInt(ans.replace(/[^0-9]/g, ''), 10);
                if (digit === 3) return { ok: true, msg: "Correct" };
                return { ok: false, msg: `Count mismatch: '${ans}' is incorrect. Re-examine every single character in the word.` };
            }
        }
    ];

    for (let i = 0; i < experiments.length; i++) {
        const exp = experiments[i];
        console.log(`------------------------------------------------------------------------`);
        console.log(`>>> ${exp.name}`);
        console.log(`Question: "${exp.question}"`);
        console.log(`------------------------------------------------------------------------`);

        // Build base continuous stream
        let stream = `<|im_start|>system
You are a precise, deterministic reasoning engine.
Follow instructions strictly. Do not invent facts.
<|im_end|>
<|im_start|>user
${exp.question}
<|im_end|>
<|im_start|>assistant
`;

        let turn = 1;
        const maxTurns = 3;
        let solved = false;

        while (turn <= maxTurns && !solved) {
            console.log(`\n--- [Turn ${turn}] Invoking Model at Continuous Stream Pointer ---`);
            const t0 = Date.now();
            const res = await queryStream(stream, { n_predict: 100 });
            const elapsed = Date.now() - t0;

            const generated = res.content.trim();
            console.log(`Generation Time: ${elapsed}ms | Tokens Evaluated (Delta): ${res.tokens_evaluated} | Tokens Reused in KV: ${res.tokens_cached || 0}`);
            console.log(`Model Response:\n>>> "${generated}"`);

            // Attach model's generation directly to the continuous stream (append pointer)
            stream += generated;

            // Evaluate without human intervention
            const evalResult = exp.evaluator(generated);
            console.log(`Evaluation Result: ${evalResult.ok ? "PASS" : "FAIL"} (${evalResult.msg})`);

            if (evalResult.ok) {
                console.log(`>>> [SUCCESS in Turn ${turn}]`);
                solved = true;
                break;
            } else {
                if (turn < maxTurns) {
                    console.log(`>>> [Injecting Pointer + Error DIRECTLY into Continuous KV Stream]`);
                    // We append the error directly as the next token stream sequence!
                    const errorAttachment = `\n<error_feedback>\n${evalResult.msg}\n</error_feedback>\n<|im_start|>assistant\n`;
                    stream += errorAttachment;
                }
            }
            turn++;
        }

        console.log(`\nFinal Outcome for ${exp.name}: ${solved ? "RESOLVED" : "UNRESOLVED"}\n`);
    }

    console.log("========================================================================");
    console.log(" EXPERIMENT COMPLETED");
    console.log("========================================================================");
}

runAutonomousPointerTest().catch(console.error);
