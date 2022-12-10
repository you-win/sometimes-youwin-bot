fn main() {
    discord_info();
    twitch_info();
}

fn discord_info() {
    handle_vars(&["DISCORD_TOKEN", "DISCORD_GUILD_ID"]);
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
