const http = require('http');
const { execSync } = require('child_process');

function queryLlama(prompt, options = {}) {
    return new Promise((resolve, reject) => {
        const payload = JSON.stringify({
            prompt: prompt,
            n_predict: options.n_predict || 300,
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

function executePythonSafely(code) {
    try {
        const fs = require('fs');
        fs.writeFileSync('/tmp/omni_test_code.py', code);
        const stdout = execSync('python3 /tmp/omni_test_code.py', { timeout: 4000, encoding: 'utf8' });
        return { success: true, output: stdout };
    } catch (err) {
        return { 
            success: false, 
            output: (err.stderr || err.message || err.stdout).toString().trim() 
        };
    }
}

function extractCode(text) {
    const match = text.match(/```python([\s\S]*?)```/);
    if (match) return match[1].trim();
    const match2 = text.match(/```([\s\S]*?)```/);
    if (match2) return match2[1].trim();
    return text.trim();
}

async function runPythonTrapExperiment() {
    console.log("=================================================================");
    console.log("  EXPERIMENT: In-KV Cache Semantic Trap & Execution Feedback");
    console.log("=================================================================\n");

    // The Trap: Linguistic ambiguity with subtle Python type hazard
    // We give a data structure where one record looks like an int string, but one is Roman numeral or mixed format,
    // causing an inevitable ValueError or TypeError if parsed naively with int().
    const systemPrompt = `<|im_start|>system
You are an expert autonomous Python coding agent.
You must think step-by-step inside <thought>...</thought> before writing the code.
Write executable Python code enclosed inside \`\`\`python ... \`\`\` blocks.
Your code must run directly, solve the problem, and print the final result to stdout.
<|im_end|>
<|im_start|>user
We have a list of raw transaction amount tokens:
data = ["150", "280", "1,200", "500", "O", "350"]
Notice: some values might be slightly irregular due to OCR scanning.

Task: Calculate the exact integer sum of all valid transactions.
Write a python script that computes this and prints: "Total: <sum>".
<|im_end|>
<|im_start|>assistant
`;

    console.log(">>> [Round 1] Presenting the task & trap to the model...");
    const t1 = Date.now();
    const res1 = await queryLlama(systemPrompt, { n_predict: 250 });
    const elapsed1 = Date.now() - t1;

    console.log(`\n[Round 1 Model Output] (${elapsed1}ms, Prefill Tokens: ${res1.tokens_evaluated}, Cached: ${res1.tokens_cached || 0}):`);
    console.log("-----------------------------------------------------------------");
    console.log(res1.content.trim());
    console.log("-----------------------------------------------------------------");

    const code1 = extractCode(res1.content);
    console.log("\n>>> [Executing Round 1 Code on local Python 3 engine]...");
    const execRes1 = executePythonSafely(code1);

    console.log(`Execution Success: ${execRes1.success}`);
    console.log("Terminal Output / Error:\n" + execRes1.output);

    if (execRes1.success) {
        console.log("\n[Note: Model miraculously bypassed the trap on attempt 1!]");
        return;
    }

    console.log("\n=================================================================");
    console.log(">>> [Round 2] Injecting Real Python Traceback DIRECTLY into KV Cache Stream");
    console.log("=================================================================\n");

    // Continuous KV injection:
    // Prompt = systemPrompt + res1.content + <execution_failure> + assistant prompt
    const continuousPromptWithTraceback = systemPrompt + res1.content + `
<execution_failure>
${execRes1.output}
</execution_failure>
<|im_start|>assistant
<thought>
`;

    const t2 = Date.now();
    const res2 = await queryLlama(continuousPromptWithTraceback, { n_predict: 350 });
    const elapsed2 = Date.now() - t2;

    console.log(`\n[Round 2 In-Cache Correction Output] (${elapsed2}ms, Delta Tokens: ${res2.tokens_evaluated}, Reused Cached Tokens: ${res2.tokens_cached}):`);
    console.log("-----------------------------------------------------------------");
    console.log("<thought>\n" + res2.content.trim());
    console.log("-----------------------------------------------------------------");

    const code2 = extractCode(res2.content);
    console.log("\n>>> [Executing Round 2 Corrected Code on local Python 3 engine]...");
    const execRes2 = executePythonSafely(code2);

    console.log(`Round 2 Execution Success: ${execRes2.success}`);
    console.log("Terminal Output / Result:\n" + execRes2.output);
}

runPythonTrapExperiment().catch(console.error);
