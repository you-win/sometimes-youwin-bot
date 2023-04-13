fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    const API_KEY: &str = "SYWB_SERVER_API_KEY";
    println!(
        "cargo:rustc-env={}={}",
        API_KEY,
        std::env::var(API_KEY).unwrap()
    )
}
