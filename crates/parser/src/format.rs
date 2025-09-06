use crate::ast::Program;

pub fn format(program: Program) -> anyhow::Result<String> {
    Ok(program.to_string())
}

pub trait Formatter {}
