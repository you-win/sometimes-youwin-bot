use std::io::BufWriter;

use rand::Rng;

/// Ping pong.
pub fn ping() -> String {
    "pong".into()
}

/// Receive a username and reply appropriately.
pub fn whoami(name: &String) -> String {
    format!("You are {name}!")
}

/// Reply with a clap emoji.
pub fn high_five() -> String {
    "👏".into()
}

/// Similar to how `cowsay` works, take a message and make it fancy.
pub fn ferris_say(text: &String) -> String {
    let buffer = vec![];
    let mut writer = BufWriter::new(buffer);

    // TODO make the max_width configurable?
    if let Err(e) = ferris_says::say(text.as_bytes(), 36, &mut writer) {
        return format!("Ferris wasn't able to say anything: {e}");
    }

    match String::from_utf8(writer.buffer().to_vec()) {
        Ok(v) => v,
        Err(e) => format!("Ferris wasn't able to say anything: {e}"),
    }
}

/// Roll a dice with the given number of sides. The number of sides must always
/// be equal to or greater than 1.
pub fn roll(mut sides: u64) -> u64 {
    let mut rng = rand::thread_rng();

    if sides < 2 {
        sides = 2;
    }

    rng.gen_range(1..=sides)
}

pub fn lurk(name: &String) -> String {
    format!("You are now lurking, {}", name)
}

/// Returns public fields from the config.
// pub async fn config() -> String {
//     // let config = crate::CONFIG.read().await;

//     // format!(
//     //     "max_message_width: {:?}\nreaction_roles: {:?}",
//     //     config.max_message_width, config.reaction_roles
//     // )

//     format!("max_message_width: {:?}\nreaction_roles: {:?}", 36, "eh")
// }

pub fn reload_config() {
    //
}
