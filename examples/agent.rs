//! A hierarchical budget for a small agent workflow.

use std::time::Duration;

use budget_context::{Budget, Resource};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tokens = Resource::new("llm.tokens")?;
    let requests = Resource::new("llm.requests")?;
    let tools = Resource::new("agent.tool_calls")?;

    let task = Budget::builder()
        .name("task")
        .limit(tokens.clone(), 100_000)
        .limit(requests.clone(), 30)
        .limit(tools.clone(), 50)
        .deadline_after(Duration::from_secs(300))
        .build()?;

    let researcher = task
        .child()
        .name("researcher")
        .limit(tokens.clone(), 25_000)
        .build()?;

    let call = researcher.reserve_many([(&requests, 1), (&tokens, 8_000)])?;
    call.commit([(&requests, 1), (&tokens, 3_421)])?;

    println!("{:#?}", researcher.snapshot());
    Ok(())
}
