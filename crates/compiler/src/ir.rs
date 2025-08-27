use std::collections::HashMap;

use ayysee_parser::ast::BinaryOpcode;

#[derive(Debug, Clone)]
pub enum VarOrConst {
    Var(VarId),
    External(String),
    Const(f64),
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct BlockId(usize);

#[derive(Default)]
pub struct Program {
    pub blocks: Vec<Block>,
}

#[derive(Default)]
pub struct Block {
    pub instructions: Vec<Instruction>,
    pub prev: Vec<BlockId>,
    pub next: Vec<BlockId>,
}

pub enum Instruction {
    Assignment {
        id: VarId,
        value: VarValue,
    },
    Branch {
        // Variable that stores the 0 (false) or != 0 (true) that will be used to decide where to jump to.
        cond: VarId,
        // Block where we jump to, when cond is true
        true_block: BlockId,
        // Block where we jump to, when cond is false
        false_block: BlockId,
    },
}

impl std::fmt::Debug for Instruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Instruction::Assignment { id, value } => {
                write!(f, "v_{} = {:?}", id.0, value)
            }
            Instruction::Branch {
                cond,
                true_block,
                false_block,
            } => {
                write!(
                    f,
                    "if {:?} {{ jump({:?} }} else {{ jump {:?} }}",
                    cond, true_block, false_block
                )
            }
        }
    }
}

impl std::fmt::Debug for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for ins in &self.instructions {
            writeln!(f, "{:?}", ins)?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for Program {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, block) in self.blocks.iter().enumerate() {
            writeln!(f, "Block {i}")?;
            write!(f, "{:?}", block)?;
        }
        Ok(())
    }
}

#[derive(Debug, Copy, Clone)]
pub struct VarId(usize);

#[derive(Debug, Clone)]
pub enum VarValue {
    Single(VarOrConst),
    BinaryOp {
        lhs: VarOrConst,
        op: BinaryOpcode,
        rhs: VarOrConst,
    },
    Call {
        name: String,
        args: Vec<VarOrConst>,
    },
}

#[derive(Default)]
struct State {
    defs: HashMap<String, HashMap<BlockId, VarId>>,
    vars: Vec<VarValue>,
    program: Program,
}

impl State {
    fn new_block(&mut self) -> BlockId {
        self.program.blocks.push(Block::default());
        BlockId(self.program.blocks.len() - 1)
    }

    fn assign(&mut self, block: BlockId, name: &str, v: VarId) {
        self.defs
            .entry(name.to_string())
            .or_default()
            .insert(block, v);
    }

    fn assign_external(&mut self, block: BlockId, name: &str) {
        let id = self.add_variable(block, VarValue::Single(VarOrConst::External(name.into())));
        self.assign(block, name, id)
    }

    fn add_variable(&mut self, block: BlockId, value: VarValue) -> VarId {
        let id = VarId(self.vars.len());
        self.vars.push(value.clone());
        // TODO: Implement IndexSlice for Block?
        self.program.blocks[block.0]
            .instructions
            .push(Instruction::Assignment { id, value });
        id
    }

    fn read_variable(&self, block: BlockId, name: &str) -> VarId {
        // TODO: handle unwrap correctly
        *self.defs.get(name).unwrap().get(&block).unwrap()
    }
}

// TODO: rename to IR
pub fn generate_program(program: ayysee_parser::ast::Program) -> anyhow::Result<String> {
    let ir = generate_ir(program)?;
    println!("{:?}", ir);
    Ok("s d0 Setting 1".to_owned())
}

pub fn generate_ir(program: ayysee_parser::ast::Program) -> anyhow::Result<Program> {
    let mut state = State::default();
    let block = state.new_block();
    state.assign_external(block, "d0");
    state.assign_external(block, "Setting");

    for stmt in &program.statements {
        println!("{:?}", stmt);
        match stmt {
            ayysee_parser::ast::Statement::FunctionCall {
                identifier,
                arguments,
            } => {
                let args: Vec<VarOrConst> = arguments
                    .iter()
                    .map(|a| process_expr(&mut state, block, a))
                    .collect();
                let id = state.add_variable(
                    block,
                    VarValue::Call {
                        name: identifier.to_string(),
                        args,
                    },
                );
            }
            ayysee_parser::ast::Statement::Definition {
                identifier,
                expression,
            } => {
                let v = process_expr(&mut state, block, &expression);
                let id = match v {
                    VarOrConst::Const(_) => state.add_variable(block, VarValue::Single(v)),
                    VarOrConst::Var(id) => id,
                    VarOrConst::External(_) => state.add_variable(block, VarValue::Single(v)),
                };
                state.assign(block, identifier.as_ref(), id);
            }
            _ => {
                anyhow::bail!("unimplemented statement");
            }
        }
    }
    Ok(state.program)
}

fn process_expr(state: &mut State, block: BlockId, expr: &ayysee_parser::ast::Expr) -> VarOrConst {
    match expr {
        ayysee_parser::ast::Expr::Constant(v) => VarOrConst::Const(v.into()),
        ayysee_parser::ast::Expr::Identifier(ident) => {
            VarOrConst::Var(state.read_variable(block, ident.as_ref()))
        }
        ayysee_parser::ast::Expr::BinaryOp(lhs_expr, op, rhs_expr) => {
            let lhs = process_expr(state, block, lhs_expr);
            let rhs = process_expr(state, block, rhs_expr);
            VarOrConst::Var(state.add_variable(block, VarValue::BinaryOp { lhs, op: *op, rhs }))
        }
        ayysee_parser::ast::Expr::UnaryOp(_, _) => todo!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate_program_ng;
    use crate::simulator::Simulator;
    use ayysee_parser::grammar::ProgramParser;
    use stationeers_mips::types::{Device, DeviceVariable};
    use std::str::FromStr;
    use test_log::test;

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
    fn test_simple_store() {
        let parser = ProgramParser::new();
        let parsed = parser
            .parse(
                r"
                store(d0, Setting, 1);
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

    #[test]
    fn test_simple_variable() {
        let parser = ProgramParser::new();
        let parsed = parser
            .parse(
                r"
                let x = 1;
                let y = x + 2;
                store(d0, Setting, y);
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
