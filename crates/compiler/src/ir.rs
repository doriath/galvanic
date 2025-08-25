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
    Ok("move r0 1\ns d0 Setting r0".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate_program_ng;
    use crate::simulator::Simulator;
    use ayysee_parser::grammar::ProgramParser;
    use stationeers_mips::types::{Device, DeviceVariable};
    use std::str::FromStr;

    fn parse_mips(
        program: &str,
    ) -> anyhow::Result<std::vec::Vec<stationeers_mips::instructions::Instruction>> {
        let mut ret = vec![];
        for line in program.lines() {
            let line = line.trim();
            ret.push(line.parse()?)
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
        println!("{}", mips);
        let instructions = parse_mips(&mips).unwrap();
        let mut simulator = Simulator::new(instructions);
        assert_eq!(simulator.tick(), crate::simulator::TickResult::End);
        assert_eq!(simulator.read(Device::D0, DeviceVariable::Setting), 1.0);
    }
}
