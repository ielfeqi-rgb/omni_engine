use omni_engine::sandbox::{LuaSandboxRunner, MemoryVfs};
use std::sync::Arc;

fn create_sandbox() -> LuaSandboxRunner {
    let vfs = Arc::new(MemoryVfs::new());
    LuaSandboxRunner::new(vfs)
}

#[test]
fn test_infinite_loop() {
    let sandbox = create_sandbox();
    let code = "
        print('Starting infinite loop...')
        local count = 0
        while true do
            count = count + 1
        end
    ";
    
    println!("--- Testing Infinite Loop ---");
    let result = sandbox.run_script(code);
    println!("Success: {}", result.success);
    println!("Error: {:?}", result.error);
    println!("Output: {}", result.output_log);
    assert!(!result.success);
}

#[test]
fn test_memory_bomb() {
    let sandbox = create_sandbox();
    let code = "
        print('Starting memory bomb...')
        local s = 'A'
        while true do
            s = s .. s
        end
    ";
    
    println!("--- Testing Memory Bomb ---");
    let result = sandbox.run_script(code);
    println!("Success: {}", result.success);
    println!("Error: {:?}", result.error);
    println!("Output: {}", result.output_log);
    assert!(!result.success);
}

#[test]
fn test_os_execution() {
    let sandbox = create_sandbox();
    let code = "
        print('Attempting to format drive...')
        os.execute('rm -rf /')
    ";
    
    println!("--- Testing OS Execution ---");
    let result = sandbox.run_script(code);
    println!("Success: {}", result.success);
    println!("Error: {:?}", result.error);
    println!("Output: {}", result.output_log);
    assert!(!result.success);
}
