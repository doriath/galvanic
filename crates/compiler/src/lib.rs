pub mod ir;
pub mod simulator;

/// Generates the MIPS assemby based on ayysee language.
pub fn generate_program(program: ayysee_parser::ast::Program) -> anyhow::Result<String> {
    Ok(crate::ir::generate_program(program)?.to_string())
}
