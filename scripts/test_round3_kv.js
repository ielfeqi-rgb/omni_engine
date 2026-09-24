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

async function runRound3() {
    console.log("=================================================================");
    console.log("  ROUND 3: Re-feeding the REAL Python error into continuous KV");
    console.log("=================================================================\n");

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
`;

    // Reconstruct full stream up to Round 2 error
    const round1Assistant = `<thought>
To solve this problem, we need to:
1. Read the content of the log file.
2. Parse the JSON data from the log file.
3. Extract the 'active_sessions' value.
4. Print the result in the required format.
</thought>

\`\`\`python
import json

with open('/tmp/server_raw_event.log', 'r') as file:
    log_content = file.read()

data = json.loads(log_content)
active_sessions = data['active_sessions']
print(f"Active Sessions: {active_sessions}")
\`\`\``;

    const round1Traceback = `Traceback (most recent call last):
  File "/tmp/omni_trap_test.py", line 7, in <module>
    data = json.loads(log_content)
json.decoder.JSONDecodeError: Expecting property name enclosed in double quotes: line 1 column 2 (char 1)`;

    const round2Assistant = `<thought>
The error occurs because the JSON data in the log file is not properly formatted. The JSON parser expects properties enclosed in double quotes, but the log file contains properties without quotes.
To fix this, we need to ensure that all properties in the JSON data are enclosed in double quotes. We can do this by modifying the log file or by parsing the JSON data manually.
</thought>

\`\`\`python
import json

with open('/tmp/server_raw_event.log', 'r') as file:
    log_content = file.read()

modified_log_content = log_content.replace('active_sessions:', '"active_sessions":')
data = json.loads(modified_log_content)
active_sessions = data['active_sessions']
print(f"Active Sessions: {active_sessions}")
\`\`\``;

    const round2Traceback = `Traceback (most recent call last):
  File "/tmp/omni_trap_test.py", line 8, in <module>
    data = json.loads(modified_log_content)
json.decoder.JSONDecodeError: Expecting property name enclosed in double quotes: line 1 column 2 (char 1)`;

    // Inject exact Round 2 traceback into the stream
    const continuousPromptRound3 = systemPrompt + 
        round1Assistant + "\n<execution_error>\n" + round1Traceback + "\n</execution_error>\n<|im_start|>assistant\n" +
        round2Assistant + "\n<execution_error>\n" + round2Traceback + "\n</execution_error>\n<|im_start|>assistant\n<thought>\n";

    console.log(">>> Sending continuous stream with both failures to observe Round 3 thinking...");
    const t0 = Date.now();
    const res3 = await queryLlama(continuousPromptRound3, { n_predict: 350 });
    const elapsed = Date.now() - t0;

    console.log(`\n[Round 3 Output] (${elapsed}ms, Prefill Tokens: ${res3.tokens_evaluated}, Cached Reused: ${res3.tokens_cached}):`);
    console.log("-----------------------------------------------------------------");
    console.log("<thought>\n" + res3.content.trim());
    console.log("-----------------------------------------------------------------");

    const code3 = extractCode(res3.content);
    console.log("\n>>> [Executing Round 3 Code on Linux Python 3 Engine]...");
    const execRes3 = executePythonSafely(code3);

    console.log(`Execution Success: ${execRes3.success}`);
    console.log("Terminal Output / Result:\n" + execRes3.output);
}

runRound3().catch(console.error);
