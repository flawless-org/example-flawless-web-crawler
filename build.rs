fn main() {
    // Re-run build if workflow changed.
    println!("cargo:rerun-if-changed=workflows");
    // Builds the crawler project located in `workflows/crawler` into a Flawless WebAssembly artifact.
    // This uses a debug build for demonstration purposes, you want probably to use `build_release` instead.
    flawless_utils::build_debug("crawler");
}
