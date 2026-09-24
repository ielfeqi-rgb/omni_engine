use std::path::Path;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let bin_dir = Path::new(&manifest_dir).join("bin");
    let out_dir = std::env::var("OUT_DIR").unwrap();

    // Compile C bridge
    cc::Build::new()
        .file("src/c_bridge/llama_bridge.c")
        .include("src/c_bridge/include")
        .flag("-O3")
        .compile("omni_llama_bridge");

    // Linker directives for C bridge static library
    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rustc-link-lib=static=omni_llama_bridge");

    // Linker directives for libllama and libggml
    println!("cargo:rustc-link-search=native={}", bin_dir.display());
    println!("cargo:rustc-link-lib=dylib=llama");
    println!("cargo:rustc-link-lib=dylib=ggml");
    println!("cargo:rustc-link-lib=dylib=ggml-base");
    println!("cargo:rustc-link-lib=dylib=ggml-cpu");
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/bin");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", bin_dir.display());

    println!("cargo:rerun-if-changed=src/c_bridge/llama_bridge.c");
    println!("cargo:rerun-if-changed=src/c_bridge/include/llama.h");
}
