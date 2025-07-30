fn main() {
    #[cfg(target_os = "macos")] {
        println!("cargo:rustc-link-arg=-Wl,-exported_symbols_list,exports.exp");
    }
    
    #[cfg(target_os = "linux")] {
        println!("cargo:rustc-link-arg=-Wl,--export-dynamic");
    }
}
