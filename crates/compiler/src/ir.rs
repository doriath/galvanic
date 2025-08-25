enum VarOrConst {
    Var(i32),
    Const(f64),
}

enum Instruction {
    SetDevice {
        device_hash: VarOrConst,
        logic_hash: VarOrConst,
        value: VarOrConst,
    },
}

struct Env {}

pub fn generate_program(program: ayysee_parser::ast::Program) -> anyhow::Result<String> {
    let env = Env {};
    for stmt in &program.statements {
        match stmt {
            ayysee_parser::ast::Statement::FunctionCall {
                identifier,
                arguments,
            } => {
                if identifier.to_string() == "set".to_string() {
                    println!("works!");
                }
                if arguments.len() != 3 {}
            }
            ayysee_parser::ast::Statement::Alias { identifier, alias } => {}
            _ => {
                anyhow::bail!("unimplemented statement");
            }
        }
        println!("{:?}", stmt);
    }
    Ok("s d0 Setting 1".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate_program_ng;
    use ayysee_parser::grammar::ProgramParser;

    fn parse_mips(
        program: &str,
    ) -> anyhow::Result<std::vec::Vec<stationeers_mips::instructions::Instruction>> {
        let ret = vec![];

        for line in program.lines() {
            let line = line.trim();
        }

        Ok(ret)
    }

    #[test]
    fn test_generate_program_ng() {
        let parser = ProgramParser::new();
        let parsed = parser
            .parse(
                r"
                def d0 as GasSensor;
                set(GasSensor, Setting, 1);
                ",
            )
            .unwrap();
        let mips = generate_program_ng(parsed).unwrap();
        let instructions = parse_mips(&mips).unwrap();
        assert_eq!(mips, "s d0 Setting 1");
    }
}
