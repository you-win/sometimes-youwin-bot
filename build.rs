const BUILD_NAME: &str = "Genesis";
const LOG_LEVEL: &str = "trace";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=.git/");

    build_info();
    discord_info();
    twitch_info();
}

fn build_info() {
    println!("cargo:rustc-env=BUILD_NAME={}", BUILD_NAME);

    let sha = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .map(|x| String::from(String::from_utf8_lossy(&x.stdout)))
        .unwrap();
    println!("cargo:rustc-env=GIT_REV={}", sha);

    println!("cargo:rustc-env=LOG_LEVEL={}", LOG_LEVEL);
}

fn discord_info() {
    handle_vars(&[
        "DISCORD_TOKEN",
        "DISCORD_GUILD_ID",
        "DISCORD_BOT_DATA_CHANNEL_ID",
    ]);
}

fn twitch_info() {
    handle_vars(&[
        "TWITCH_CLIENT_ID",
        "TWITCH_CLIENT_SECRET",
        "TWITCH_REFRESH_TOKEN",
    ]);
}

fn handle_vars(keys: &[&str]) {
    for key in keys {
        match std::env::var(key) {
            Ok(val) => println!("cargo:rustc-env={}={}", key, val),
            Err(_) => panic!("Missing environment var {}", key),
        }
    }
}
