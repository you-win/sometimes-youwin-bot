use rhai::{Dynamic, Engine};

/// The max number of operations that a Rhai script can do before it is
/// forcible halted.
pub const MAX_SCRIPTING_OPS: u64 = 10_000;

pub fn execute_timed(text: impl ToString, max_time: u64) -> anyhow::Result<String> {
    let mut engine = Engine::new();

    engine.on_progress(move |count| {
        if count <= max_time {
            None
        } else {
            Some(Dynamic::UNIT)
        }
    });

    engine
        .eval::<Dynamic>(text.to_string().as_str())
        .map_err(anyhow::Error::from)
        .map(|x| x.to_string())
}

pub fn execute(text: impl ToString) -> anyhow::Result<String> {
    execute_timed(text, MAX_SCRIPTING_OPS)
}
