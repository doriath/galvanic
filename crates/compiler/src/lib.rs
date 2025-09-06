use crate::error::Result;

pub mod error;
pub mod ir;
pub mod simulator;

/// Generates the MIPS assemby based on ayysee language.
pub fn generate_program(program: ayysee_parser::ast::Program) -> Result<String> {
    Ok(crate::ir::generate_program(program).unwrap().to_string())
}
