use regex::Regex;
use rhai::{Dynamic, Engine, Locked, Module, Scope, Shared};

/// The max number of operations that a Rhai script can do before it is
/// forcible halted.
pub const MAX_SCRIPTING_OPS: u64 = 10_000;

const HEADER_TEMPLATE: &str = r"
fn sleep(n) {
    //
}
";

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
            Some(Dynamic::UNIT)
        }
    });

    let template_ast = engine.compile(HEADER_TEMPLATE)?;
    let template_module = Module::eval_ast_as_new(Scope::new(), &template_ast, &engine)?;
    engine.register_global_module(template_module.into());

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
