const http = require('http');
const { execSync } = require('child_process');
const fs = require('fs');

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
        fs.writeFileSync('/tmp/omni_trap_test.py', code);
        const stdout = execSync('python3 /tmp/omni_trap_test.py', { timeout: 4000, encoding: 'utf8' });
        return { success: true, output: stdout.trim() };
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

async function runRealTrap() {
    console.log("=================================================================");
    console.log("  EXPERIMENT 2: Guaranteed Semantic/API Trap in Python");
    console.log("=================================================================\n");

    // The Trap: Ask model to read a JSON-like log file that contains single quotes instead of double quotes,
    // and specifically guide it to use `json.loads()`.
    // Standard json.loads() strictly fails with json.decoder.JSONDecodeError!
    // The test will see if it switches to `ast.literal_eval` or regex after error injection.

    fs.writeFileSync('/tmp/server_raw_event.log', "{'status': 'ok', 'users': ['admin', 'guest'], 'active_sessions': 42}");

    const systemPrompt = `<|im_start|>system
You are an autonomous Python engineering agent.
Always provide your internal reasoning inside <thought>...</thought> before writing the code.
Output your final solution in a \`\`\`python ... \`\`\` block.
Your script must be fully self-contained, run on Linux Python 3, and print the target value.
<|im_end|>
<|im_start|>user
We have a log file at '/tmp/server_raw_event.log'.
It contains a JSON log record of current server stats.
Task: Parse this file using Python, extract 'active_sessions', and print:
"Active Sessions: <count>"
<|im_end|>
<|im_start|>assistant
<thought>
`;

    console.log(">>> [Round 1] Setting trap: Prompting model to parse '/tmp/server_raw_event.log' as JSON...");
    const t1 = Date.now();
    const res1 = await queryLlama(systemPrompt, { n_predict: 250 });
    const elapsed1 = Date.now() - t1;

    console.log(`\n[Round 1 Output] (${elapsed1}ms, Cached Tokens: ${res1.tokens_cached || 0}):`);
    console.log("-----------------------------------------------------------------");
    console.log("<thought>\n" + res1.content.trim());
    console.log("-----------------------------------------------------------------");

    const code1 = extractCode(res1.content);
    console.log("\n>>> [Executing Round 1 Code]...");
    const execRes1 = executePythonSafely(code1);

    console.log(`Round 1 Success: ${execRes1.success}`);
    console.log("Stdout / Traceback:\n" + execRes1.output);

    if (execRes1.success) {
        console.log("Model somehow avoided the trap.");
        return;
    }

    console.log("\n=================================================================");
    console.log(">>> [Round 2] Injecting Real Python Traceback DIRECTLY into Continuous KV Cache");
    console.log("=================================================================\n");

    // Continuous KV injection: Append model's previous code, then the exact Linux Python Traceback
    const continuousPrompt = systemPrompt + res1.content + `
<execution_error>
${execRes1.output}
</execution_error>
<|im_start|>assistant
<thought>
`;

    const t2 = Date.now();
    const res2 = await queryLlama(continuousPrompt, { n_predict: 300 });
    const elapsed2 = Date.now() - t2;

    console.log(`\n[Round 2 Model Thinking & Correction] (${elapsed2}ms, Delta Tokens: ${res2.tokens_evaluated}, Reused Tokens: ${res2.tokens_cached}):`);
    console.log("-----------------------------------------------------------------");
    console.log("<thought>\n" + res2.content.trim());
    console.log("-----------------------------------------------------------------");

    const code2 = extractCode(res2.content);
    console.log("\n>>> [Executing Round 2 Corrected Code]...");
    const execRes2 = executePythonSafely(code2);

    console.log(`Round 2 Success: ${execRes2.success}`);
    console.log("Final Execution Result:\n" + execRes2.output);
}

runRealTrap().catch(console.error);
