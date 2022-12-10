pub fn strip_command_prefix(text: &String) -> String {
    match text.split_once(" ") {
        Some(v) => v.1.into(),
        None => text.clone(),
    }
}
