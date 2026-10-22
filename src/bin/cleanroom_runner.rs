// ============================================================================
// [SOVEREIGN ENGINE CONSTITUTION - INVIOLABLE SAFETY DIRECTIVE]
// STRICT ARCHITECTURAL RULE:
// It is strictly forbidden, under any circumstances or for any reason whatsoever,
// to remove, comment out, bypass, or weaken `#![deny(unused_must_use)]` from
// this file or ANY other .rs file in this codebase.
// All fallible operations, Results, and Options must be explicitly handled.
// Zero silent error swallowing under the rug. This engine is built to endure for decades.
// ============================================================================
#![deny(unused_must_use)]

use omni_engine::sandbox::cleanroom::UniversalCleanroom;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

fn print_usage() {
    println!("Omni Engine - Universal Cleanroom Testing Environment");
    println!("Usage:");
    println!("  cleanroom_runner --text <file_path> [--topics <topic1,topic2>]");
    println!("  cleanroom_runner --code <file_path>");
    println!("  cleanroom_runner --interactive <command> [--inputs <in1,in2,...>] [--files <f1,f2,...>]");
    println!("  cleanroom_runner --acceptance <file_path> --goal \"<user_goal>\"");
    println!("\nExamples:");
    println!("  cleanroom_runner --text README.md --topics \"architecture,scheduler\"");
    println!("  cleanroom_runner --code workspace/Calculator.java");
    println!("  cleanroom_runner --interactive \"python3 workspace/main.py\" --inputs \"10,20,+\"");
    println!("  cleanroom_runner --acceptance workspace/calc.py --goal \"Interactive calculator with history in json\"");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        print_usage();
        return;
    }

    match args[1].as_str() {
        "--text" => {
            let file_path = &args[2];
            let content = match fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Error reading file '{}': {}", file_path, e);
                    std::process::exit(1);
                }
            };

            let mut topics = Vec::new();
            if args.len() >= 5 && args[3] == "--topics" {
                topics = args[4].split(',').map(|s| s.trim()).collect();
            }

            println!("\n🔍 [MODE 1: WRITTEN CONTENT & SPEC AUDIT]");
            println!("File: {}", file_path);
            let report = UniversalCleanroom::audit_written_content(&content, &topics);
            println!("  Passed:             {}", report.is_complete);
            println!("  Score:              {:.2}/1.00", report.score);
            println!("  Format:             {}", report.detected_format);
            println!("  Word Count:         {}", report.word_count);
            println!("  Placeholders Found: {}", report.placeholder_count);
            println!("  Sections:           {:?}", report.sections_detected);
            println!("  Covered Topics:     {:?}", report.covered_topics);
            println!("  Missing Topics:     {:?}", report.missing_topics);
            println!("  Critique:           {}", report.critique);

            if !report.is_complete {
                std::process::exit(1);
            }
        }
        "--code" => {
            let file_path = &args[2];
            let bytes = match fs::read(file_path) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Error reading code file '{}': {}", file_path, e);
                    std::process::exit(1);
                }
            };

            println!("\n🔬 [MODE 2: LANGUAGE-AGNOSTIC CODE & AST AUDIT]");
            println!("File: {}", file_path);
            let report = UniversalCleanroom::audit_code_syntax(file_path, &bytes);
            println!("  Verification Cmd:   {}", report.verification_command);
            println!("  Valid / Passed:     {}", report.is_valid);
            println!("  Exit Code:          {:?}", report.exit_code);

            if let Some(line) = report.error_line {
                println!("  Target Error Line:  #{}", line);
            }
            if let Some(ref err) = report.error_message {
                println!("  Error Diagnostic:   {}", err);
            }
            if let Some(ref snippet) = report.broken_snippet {
                println!("  Broken AST Snippet:\n{}", snippet);
            }

            if !report.is_valid {
                std::process::exit(1);
            }
        }
        "--interactive" => {
            let cmd = &args[2];
            let mut inputs = Vec::new();
            let mut staged_paths = Vec::new();

            let mut i = 3;
            while i < args.len() {
                if args[i] == "--inputs" && i + 1 < args.len() {
                    inputs = args[i + 1].split(',').map(|s| s.trim()).collect();
                    i += 2;
                } else if args[i] == "--files" && i + 1 < args.len() {
                    staged_paths = args[i + 1].split(',').map(|s| s.trim().to_string()).collect();
                    i += 2;
                } else {
                    i += 1;
                }
            }

            let mut staged_files = Vec::new();
            for p in staged_paths {
                if let Ok(bytes) = fs::read(&p) {
                    let base_name = std::path::Path::new(&p).file_name().map(PathBuf::from).unwrap_or_else(|| PathBuf::from(&p));
                    staged_files.push((base_name, bytes));
                }
            }

            println!("\n⚡ [MODE 3: REAL INTERACTIVE EXECUTION HARNESS]");
            println!("Command: {}", cmd);
            println!("Simulated Inputs: {:?}", inputs);

            let report = UniversalCleanroom::execute_interactive(
                cmd,
                &staged_files,
                &inputs,
                Duration::from_secs(10),
            );

            println!("  Success:            {}", report.success);
            println!("  Exit Code:          {:?}", report.exit_code);
            println!("  Timed Out:          {}", report.timed_out);
            println!("\n--- STDOUT ---\n{}", report.full_stdout);
            if !report.full_stderr.is_empty() {
                println!("--- STDERR ---\n{}", report.full_stderr);
            }

            if !report.success {
                std::process::exit(1);
            }
        }
        "--acceptance" => {
            let file_path = &args[2];
            let content = match fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Error reading file '{}': {}", file_path, e);
                    std::process::exit(1);
                }
            };
            let mut user_goal = String::new();
            let mut i = 3;
            while i < args.len() {
                if (args[i] == "--goal" || args[i] == "-g") && i + 1 < args.len() {
                    user_goal = args[i + 1].clone();
                    i += 2;
                } else {
                    i += 1;
                }
            }

            println!("\n📋 [MODE 4: ACCEPTANCE CRITERIA & DELTA AUDIT]");
            println!("File: {}", file_path);
            println!("Goal: {}", user_goal);
            let report = UniversalCleanroom::audit_acceptance_criteria(file_path, &content, &user_goal);
            println!("  Accepted:             {}", report.is_accepted);
            println!("  Covered Requirements: {:?}", report.covered_requirements);
            println!("  Missing Requirements: {:?}", report.missing_requirements);
            println!("  Delta Summary:        {}", report.delta_summary);
            println!("  Recommendation:       {}", report.recommendation);

            if !report.is_accepted {
                std::process::exit(1);
            }
        }
        other => {
            eprintln!("Unknown mode: {}", other);
            print_usage();
            std::process::exit(1);
        }
    }
}
