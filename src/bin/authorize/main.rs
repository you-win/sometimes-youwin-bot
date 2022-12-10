use anyhow::Result;

use sometimes_youwin;

fn main() -> Result<()> {
    sometimes_youwin::debug::hello_world();
    println!("Running authorize server");

    Ok(())
}
