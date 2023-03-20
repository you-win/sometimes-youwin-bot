pub const MAX_SCRIPTING_OPS: u64 = 10_000;

pub fn execute_timed(text: impl ToString, max_time: u64) {
    todo!()
}

pub fn execute(text: impl ToString) {
    execute_timed(text, MAX_SCRIPTING_OPS);
}
