fn main() {
    println!(
        "Starting build {} with rev {}",
        sometimes_youwin::BUILD_NAME,
        sometimes_youwin::GIT_REV
    );

    env_logger::Builder::new()
        .parse_filters(format!("warn,sometimes_youwin={}", sometimes_youwin::LOG_LEVEL).as_str())
        .init();

    sometimes_youwin::discord::run_bot();
}
