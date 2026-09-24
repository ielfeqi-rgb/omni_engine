const http = require('http');

function queryStream(prompt, options = {}) {
    return new Promise((resolve, reject) => {
        const payload = JSON.stringify({
            prompt: prompt,
            n_predict: options.n_predict || 250,
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

async function runHardPointerSuite() {
    console.log("========================================================================");
    console.log("  STRICT FAILURE & POINTER RECOVERY SUITE (OBSERVING IN-CACHE BEHAVIOR)");
    console.log("========================================================================\n");

    const experiments = [
        {
            name: "EXP 1: Easy Logic (Reverse Even Filter)",
            prompt: "Given array: [3, 8, 12, 5, 14, 7, 20]. Extract only the even numbers and list them in reverse order separated by commas. Reply strictly with the numbers only.",
            evaluator: (ans) => {
                const clean = ans.replace(/[^0-9,]/g, '').trim();
                if (clean === "20,14,12,8") return { ok: true, msg: "Correct" };
                return { ok: false, msg: `Unmatched: Expected exact string '20,14,12,8', got '${ans}'.` };
            }
        },
        {
            name: "EXP 2: Guaranteed Failure (Multi-step String Mutation)",
            // Almost 100% fail rate for 1.5B on turn 1:
            // "Take word 'PYTHON'. Shift each letter +3 forward in alphabet (P->S, Y->B, T->W, H->K, O->R, N->Q) -> 'SBWKRQ'. Then reverse it -> 'QRKWBS'."
            prompt: "Take the uppercase word 'PYTHON'. Shift each letter forward by 3 positions in the alphabet (with wrap-around: Y->B). Then reverse the resulting string. Reply with strictly the final 6-letter uppercase string only.",
            evaluator: (ans) => {
                const clean = ans.replace(/[^A-Z]/g, '').trim();
                // P(+3)=S, Y(+3)=B, T(+3)=W, H(+3)=K, O(+3)=R, N(+3)=Q -> SBWKRQ -> reversed: QRKWBS
                if (clean === "QRKWBS") return { ok: true, msg: "Correct" };
                return { 
                    ok: false, 
                    msg: `Failure: Got '${clean}'. Correct transformation rule: P->S, Y->B, T->W, H->K, O->R, N->Q makes 'SBWKRQ'. Then reversed is 'QRKWBS'.` 
                };
            }
        },
        {
            name: "EXP 3: Subtle Constraint Challenge (Calendar Offset)",
            prompt: "If May 1st is a Wednesday, what day of the week is June 1st of the same year (assuming a non-leap year)? Reply with strictly the day name only.",
            evaluator: (ans) => {
                const clean = ans.replace(/[^a-zA-Z]/g, '').trim().toLowerCase();
                // May has 31 days. 31 % 7 = 3. Wednesday + 3 days = Saturday.
                if (clean === "saturday") return { ok: true, msg: "Correct" };
                return { 
                    ok: false, 
                    msg: `Incorrect day: You answered '${ans}'. Remember May has 31 days. Calculate 31 mod 7 offset from Wednesday.` 
                };
            }
        }
    ];

    for (let i = 0; i < experiments.length; i++) {
        const exp = experiments[i];
        console.log(`------------------------------------------------------------------------`);
        console.log(`>>> ${exp.name}`);
        console.log(`------------------------------------------------------------------------`);

        let stream = `<|im_start|>system
You are an ultra-precise reasoning engine. Follow instructions and error corrections strictly.
<|im_end|>
<|im_start|>user
${exp.prompt}
<|im_end|>
<|im_start|>assistant
`;

        let turn = 1;
        const maxTurns = 3;
        let solved = false;

        while (turn <= maxTurns && !solved) {
            console.log(`\n--- [Turn ${turn}] Generating from Pointer ---`);
            const t0 = Date.now();
            const res = await queryStream(stream, { n_predict: 120 });
            const elapsed = Date.now() - t0;

            const generated = res.content.trim();
            console.log(`Time: ${elapsed}ms | Delta Tokens: ${res.tokens_evaluated} | Reused in KV: ${res.tokens_cached || 0}`);
            console.log(`Model Response: "${generated}"`);

            stream += generated;

            const evalResult = exp.evaluator(generated);
            console.log(`Eval: ${evalResult.ok ? "PASS" : "FAIL"}`);

            if (evalResult.ok) {
                console.log(`>>> [SUCCESS in Turn ${turn}]`);
                solved = true;
                break;
            } else {
                console.log(`>>> Trace Error: ${evalResult.msg}`);
                if (turn < maxTurns) {
                    console.log(`>>> Appending Error to KV Stream Pointer...`);
                    stream += `\n<error>\n${evalResult.msg}\n</error>\n<|im_start|>assistant\n`;
                }
            }
            turn++;
        }

        console.log(`\nFinal Outcome: ${solved ? "SUCCESS" : "FAILED"}\n`);
    }
}

runHardPointerSuite().catch(console.error);
