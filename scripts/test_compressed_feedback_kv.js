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
        fs.writeFileSync('/tmp/omni_compressed_test.py', code);
        const stdout = execSync('python3 /tmp/omni_compressed_test.py', { timeout: 4000, encoding: 'utf8' });
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

async function runCompressedExperiment() {
    console.log("=================================================================");
    console.log("  EXPERIMENT: In-KV Lossless Semantic Compression of History");
    console.log("=================================================================\n");

    const historySection = [
        "[Compressed Execution History]",
        "- Attempt 1: json.loads() failed with JSONDecodeError (the file uses single quotes, not JSON double quotes).",
        "- Attempt 2: Partial replace of 'active_sessions' failed (the file is formatted as Python dict syntax).",
        "- Ground Truth File Content: \"{'status': 'ok', 'users': ['admin', 'guest'], 'active_sessions': 42}\"",
        "- Guidance: Do NOT use json.loads. Use Python's ast.literal_eval or regular expressions (re)."
    ].join("\n");

    const prompt = `<|im_start|>system
You are an autonomous Python engineering agent.
Always provide your internal reasoning inside <thought>...</thought> before writing the code.
Output your final solution in a \`\`\`python ... \`\`\` block.
Your script must be fully self-contained, run on Linux Python 3, and print the target value.
<|im_end|>
<|im_start|>user
Task: Parse '/tmp/server_raw_event.log', extract 'active_sessions', and print: "Active Sessions: <count>".

${historySection}
<|im_end|>
<|im_start|>assistant
<thought>
`;

    console.log(">>> Sending compressed semantic feedback to model...");
    const t0 = Date.now();
    const res = await queryLlama(prompt, { n_predict: 300 });
    const elapsed = Date.now() - t0;

    console.log(`\n[Model Response] (${elapsed}ms, Prefill Tokens: ${res.tokens_evaluated}, Cached: ${res.tokens_cached || 0}):`);
    console.log("-----------------------------------------------------------------");
    console.log("<thought>\n" + res.content.trim());
    console.log("-----------------------------------------------------------------");

    const code = extractCode(res.content);
    console.log("\n>>> [Executing Generated Code on Python 3 Engine]...");
    const execRes = executePythonSafely(code);

    console.log(`Execution Success: ${execRes.success}`);
    console.log("Terminal Output / Result:\n" + execRes.output);
}

runCompressedExperiment().catch(console.error);
