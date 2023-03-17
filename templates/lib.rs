use space_lib::{space, Result};
use serde::{Serialize, Deserialize};

#[derive(Deserialize)]
struct Input {
    value: usize,
    name: String,
}

#[derive(Serialize)]
struct Output {
    value: usize,
    name: String,
}

#[space]
fn main(input: Input) -> Result<Output> {
    let output = Output {
        value: input.value * 2,
        name: input.name.chars().rev().collect(),
    };
    Ok(output)
}
