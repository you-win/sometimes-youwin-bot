use std::str::FromStr;

use rhai::{plugin::*, Dynamic, Engine, Locked, Module, Shared};

/// The max number of operations that a Rhai script can do before it is
/// forcible halted.
pub const MAX_SCRIPTING_OPS: u64 = 10_000;

#[export_module]
mod bot_prelude {
    #[rhai_fn(global)]
    pub fn sleep(_n: i64) {
        // Intentionally left blank
    }
}

pub fn execute_timed(text: impl AsRef<str>, max_time: u64) -> anyhow::Result<String> {
    let mut engine = Engine::new();

    let out = Shared::new(Locked::new(Vec::new()));
    {
        let engine_out = out.clone();
        engine.on_print(move |t| {
            if let Ok(mut v) = engine_out.write() {
                v.push(t.to_string());
            }
        });
    }
    // Intentionally disabled
    engine.on_debug(|_, _, _| {});

    engine.on_progress(move |count| {
        if count <= max_time {
            None
        } else {
            Some(Dynamic::from_str("Too many operations, bailing out.").unwrap_or_default())
        }
    });

    let bot_prelude = exported_module!(bot_prelude);
    engine.register_global_module(bot_prelude.into());

    let script_ret = engine
        .eval::<Dynamic>(text.as_ref())
        .map_err(anyhow::Error::from)
        .map(|x| x.to_string())?;
    let print_ret = if let Ok(v) = out.read() {
        v.join("\n")
    } else {
        "Failed to read print logs".into()
    };

    Ok(format!("{print_ret}\n{script_ret}"))
}

pub fn execute(text: impl AsRef<str>) -> anyhow::Result<String> {
    execute_timed(text, MAX_SCRIPTING_OPS)
}
