use rhai::{Dynamic, Engine, Locked, Shared, Stmt};

/// The max number of operations that a Rhai script can do before it is
/// forcible halted.
pub const MAX_SCRIPTING_OPS: u64 = 10_000;

pub const SLEEP_FN: &str = "sleep";

pub fn execute_timed(text: impl ToString, max_time: u64) -> anyhow::Result<String> {
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

    let ast = engine.compile(text.to_string())?;
    if ast.statements().iter().any(|s| match s {
        Stmt::FnCall(boxed_fn_call, _) => boxed_fn_call.name.to_lowercase().as_str() == SLEEP_FN,
        _ => false,
    }) || ast.iter_functions().any(|f| f.name == SLEEP_FN)
    {
        anyhow::bail!("Blacklisted function detected, declining to run.");
    }

    let script_ret = engine
        .eval_ast::<Dynamic>(&ast)
        .map_err(anyhow::Error::from)
        .map(|x| x.to_string())?;
    let print_ret = if let Ok(v) = out.read() {
        v.join("\n")
    } else {
        "Failed to read print logs".into()
    };

    Ok(format!("{print_ret}\n{script_ret}"))
}

pub fn execute(text: impl ToString) -> anyhow::Result<String> {
    execute_timed(text, MAX_SCRIPTING_OPS)
}
