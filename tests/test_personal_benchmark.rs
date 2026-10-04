use omni_engine::native_llama::NativeLlamaModel;
use std::path::PathBuf;

fn run_test(model_name: &str, prompt: &str) {
    let model_path = PathBuf::from("models").join(model_name);
    if !model_path.exists() {
        println!("Model {} not found.", model_name);
        return;
    }
    
    println!("==================================================");
    println!("Loading {}...", model_name);
    let model = NativeLlamaModel::load(&model_path, 0).expect("Failed to load model");

    let mut ctx = model.create_context(2048, 2048, 4).expect("Context");
    println!("--- PROMPT ---");
    println!("{}", prompt);
    println!("--- RESPONSE ---");
    
    let response = ctx.generate(prompt, 512).expect("Generate");
    
    println!("{}", response);
    println!("==================================================\n");
}

#[test]
fn test_models_personally() {
    let base_prompt = "Please perform exactly three tasks:\n1. Write a Python function to reverse a string.\n2. Explain the time complexity of your function.\n3. Translate your explanation into French.";
    
    let qwen_prompt = format!("<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n", base_prompt);
    let gemma_prompt = format!("<start_of_turn>user\n{}<end_of_turn>\n<start_of_turn>model\n", base_prompt);
    
    run_test("qwen2.5-1.5b.gguf", &qwen_prompt);
    run_test("gemma-2b.gguf", &gemma_prompt);
}
