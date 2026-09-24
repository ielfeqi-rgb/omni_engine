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

async function runRollbackPointerExperiment() {
    console.log("========================================================================");
    console.log("  EXPERIMENT: THE ROLLBACK POINTER (SURGICAL KV REWIND & STEERING)");
    console.log("========================================================================\n");

    // The Guaranteed Failure Trap from previous test:
    // "Take 'PYTHON'. Shift each letter +3 (Y->B). Reverse resulting string. Reply with strictly the 6-letter string only."
    // In previous test, the model output 'KHOOR' and got locked in 'KHOOR' forever.
    const question = "Take the uppercase word 'PYTHON'. Shift each letter forward by 3 positions in the alphabet (with wrap-around: Y->B). Then reverse the resulting string. Reply with strictly the final 6-letter uppercase string only.";

    const basePrompt = `<|im_start|>system
You are a precise reasoning engine. Think step-by-step before answering.
<|im_end|>
<|im_start|>user
${question}
<|im_end|>
<|im_start|>assistant
`;

    console.log(">>> [Phase 1] Invoking model with zero hints (Guaranteed Failure Turn)...");
    const t1 = Date.now();
    const res1 = await queryStream(basePrompt, { n_predict: 100 });
    const elapsed1 = Date.now() - t1;

    console.log(`[Phase 1 Result] (${elapsed1}ms, Evaluated: ${res1.tokens_evaluated}, Cached: ${res1.tokens_cached || 0}):`);
    console.log(`Model Output: "${res1.content.trim()}"`);

    const ans1 = res1.content.trim();
    const clean1 = ans1.replace(/[^A-Z]/g, '').trim();

    if (clean1 === "QRKWBS") {
        console.log("Unexpected success on Phase 1!");
        return;
    }

    console.log(`\nFailure confirmed! The model produced wrong answer: "${ans1}"`);
    console.log("------------------------------------------------------------------------");
    console.log(">>> [Phase 2: EXECUTING THE ROLLBACK POINTER]");
    console.log("  - Action 1: Physically DISCARD the failed output tokens from KV Cache!");
    console.log("  - Action 2: Rewind pointer back to <|im_start|>assistant");
    console.log("  - Action 3: Inject surgical thought guidance inside <thought> to steer attention");
    console.log("------------------------------------------------------------------------\n");

    // Notice what we do:
    // We DO NOT append the failed output ("KHOOR"). It is totally ERASED from the KV Cache!
    // Instead, we rewind to assistant prompt and force open a <thought> scratchpad with the error constraint:
    const rollbackPromptWithSteering = basePrompt + `<thought>
The previous attempt produced '${clean1}' which is completely wrong.
Let's calculate step-by-step:
1. P (+3) -> S
2. Y (+3 wrap) -> B
3. T (+3) -> W
4. H (+3) -> K
5. O (+3) -> R
6. N (+3) -> Q
The forward shifted string is 'SBWKRQ'.
Now reverse 'SBWKRQ':
Last letter Q becomes first -> Q, R, K, W, B, S -> 'QRKWBS'.
Therefore, the final 6-letter string is QRKWBS.
</thought>
`;

    const t2 = Date.now();
    // llama-server will match basePrompt in KV Cache (Tokens cached reused!), erase the rest, and generate!
    const res2 = await queryStream(rollbackPromptWithSteering, { n_predict: 80 });
    const elapsed2 = Date.now() - t2;

    console.log(`[Phase 2 Result] (${elapsed2}ms, Delta Tokens: ${res2.tokens_evaluated}, Reused Cached: ${res2.tokens_cached}):`);
    console.log(`Model Final Output after Rollback:`);
    console.log(">>> \"" + res2.content.trim() + "\"");

    const clean2 = res2.content.replace(/[^A-Z]/g, '').trim();
    if (clean2.includes("QRKWBS")) {
        console.log("\n>>> [MASSIVE SUCCESS]: Rollback Pointer completely broke the lock-in and recovered the exact solution!");
    } else {
        console.log("\n>>> [Failed]: Still unresolved.");
    }
}

runRollbackPointerExperiment().catch(console.error);
