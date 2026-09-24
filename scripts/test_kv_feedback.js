const http = require('http');

function queryLlama(prompt, options = {}) {
    return new Promise((resolve, reject) => {
        const payload = JSON.stringify({
            prompt: prompt,
            n_predict: options.n_predict || 150,
            temperature: 0.1,
            cache_prompt: true,
            id_slot: options.id_slot !== undefined ? options.id_slot : 0,
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
                    const parsed = JSON.parse(body);
                    resolve(parsed);
                } catch (e) {
                    reject(new Error("Failed to parse JSON: " + body));
                }
            });
        });

        req.on('error', reject);
        req.write(payload);
        req.end();
    });
}

async function runExperiment() {
    console.log("=== Phase 1: Establish System Context & Goal in KV Cache ===");

    const systemPrompt = `<|im_start|>system
You are a deterministic micro-agent running directly in a terminal browser.
You have tools:
- click(id)
- type(id, text)
- finish(result)

Respond with a brief thought in <thought>...</thought> then strictly ONE tool call.
Goal: Extract the confirmation token from the server settings page.
<|im_end|>
<|im_start|>user
[Terminal Screen]
[1] [Button] "Refresh Settings"
[2] [Button] "Security Keys"
[3] [Input]  "Search configs..."
<|im_end|>
<|im_start|>assistant
`;

    const startT1 = Date.now();
    const res1 = await queryLlama(systemPrompt, { id_slot: 0, n_predict: 150 });
    const elapsed1 = Date.now() - startT1;
    console.log(`[Slot 0 Initial Response] (${elapsed1}ms):`);
    console.log(res1.content.trim());
    console.log(`Tokens evaluated (Prompt Prefill): ${res1.tokens_evaluated}, Tokens cached reused: ${res1.tokens_cached || 0}`);

    console.log("\n=== Phase 2: Injecting Error DIRECTLY into Continuous KV Cache Stream ===");
    const step1Output = res1.content.trim();
    
    // Injecting feedback directly as the immediate observation
    const continuousPromptWithFeedback = systemPrompt + step1Output + `
<tool_response>
Error: Action failed! Permission Denied: Cannot access Security Keys directly without 2FA unlocked. Available fallback: [1] "Refresh Settings" updates authentication status.
</tool_response>
<|im_start|>assistant
`;

    const startT2 = Date.now();
    // Prompt caching will instantly reuse all previous tokens from Step 1!
    const res2 = await queryLlama(continuousPromptWithFeedback, { id_slot: 0, n_predict: 150 });
    const elapsed2 = Date.now() - startT2;
    console.log(`[Slot 0 Post-Error In-Cache Self-Correction] (${elapsed2}ms):`);
    console.log(res2.content.trim());
    console.log(`Tokens evaluated in step 2 (Notice only the delta!): ${res2.tokens_evaluated}, Tokens cached reused: ${res2.tokens_cached || 0}`);
}

runExperiment().catch(console.error);
