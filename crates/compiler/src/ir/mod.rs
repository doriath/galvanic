mod codegen;
mod optimize;
pub mod types;

use crate::ir::codegen::generate_mips_from_ir;
use ayysee_parser::ast;
use stationeers_mips as mips;
use std::collections::{HashMap, HashSet};
pub use types::*;

struct State {
    defs: HashMap<String, HashMap<BlockId, VarId>>,
    next_var: VarId,
    program: Program,
    sealed_blocks: HashSet<BlockId>,
    unresolved_phis: HashMap<BlockId, Vec<(String, VarId, usize)>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            defs: Default::default(),
            next_var: VarId(1),
            program: Default::default(),
            sealed_blocks: Default::default(),
            unresolved_phis: Default::default(),
        }
    }
}

impl State {
    fn new_block(&mut self, sealed: bool) -> BlockId {
        let id = BlockId(self.program.blocks.len());
        self.program.blocks.push(Block::default());
        if sealed {
            self.sealed_blocks.insert(id);
        }
        id
    }

    fn seal_block(&mut self, block: BlockId) {
        if self.sealed_blocks.contains(&block) {
            return;
        }
        self.sealed_blocks.insert(block);

        // Resolve PHIs
        let phis = self.unresolved_phis.remove(&block);
        if let Some(phis) = phis {
            for (name, id, idx) in phis {
                let mut all: Vec<VarId> = vec![];
                let prevs = self.program.blocks[block.0].prev.clone();
                tracing::debug!("Sealing {:?}, prev: {:?}", block, prevs);
                for prev in &prevs {
                    let i = self.read_variable(*prev, &name).into();
                    if i != id {
                        all.push(i);
                    }
                }
                let value = VarValue::Phi(all);
                self.program.blocks[block.0].instructions[idx] =
                    Instruction::Assignment { id, value };
            }
        }

        let next = self.program.blocks[block.0].next.clone();
        for n in next {
            self.seal_block(n);
        }
    }

    fn connect_blocks(&mut self, from: BlockId, to: BlockId) {
        self.program.blocks[from.0].next.push(to);
        self.program.blocks[to.0].prev.push(from);
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

    fn next_var(&mut self) -> VarId {
        let x = self.next_var;
        self.next_var = VarId(self.next_var.0 + 1);
        x
    }

    fn add_variable(&mut self, block: BlockId, value: VarValue) -> VarId {
        let id = self.next_var();
        self.program.blocks[block.0]
            .instructions
            .push(Instruction::Assignment { id, value });
        id
    }

    fn read_variable(&mut self, block: BlockId, name: &str) -> VarId {
        if let Some(x) = self.defs.get(name).unwrap().get(&block) {
            return *x;
        }
        if !self.sealed_blocks.contains(&block) {
            tracing::debug!("Block {:?} is not sealed", block);
            let id = self.add_variable(block, VarValue::Phi(vec![]));
            self.assign(block, name, id);
            self.unresolved_phis.entry(block).or_default().push((
                name.to_string(),
                id,
                self.program.blocks[block.0].instructions.len() - 1,
            ));
            return id;
        }

        // Variable not available in this block yet
        // First, add the new variable (to ensure we don't break when cycle occurs)
        let id = self.next_var();
        // This will be changed later to real value
        self.assign(block, name, id);
        let mut all: Vec<VarId> = vec![];

        let prevs = self.program.blocks[block.0].prev.clone();
        for prev in &prevs {
            all.push(self.read_variable(*prev, name).into());
        }
        tracing::debug!(
            "reading block:{:?} name:{}: prevs{:?} all:{:?}",
            block,
            name,
            prevs,
            all
        );

        let value = if all.len() == 1 {
            all[0].clone().into()
        } else {
            VarValue::Phi(all)
        };
        self.program.blocks[block.0]
            .instructions
            .push(Instruction::Assignment { id, value });

        id
    }

    fn init(&mut self, block: BlockId) {
        self.assign_external(block, "d0");
        self.assign_external(block, "d1");
        self.assign_external(block, "d2");
        self.assign_external(block, "d3");
        self.assign_external(block, "d4");
        self.assign_external(block, "d5");
        self.assign_external(block, "Setting");
    }
}

pub fn generate_program(program: ayysee_parser::ast::Program) -> anyhow::Result<mips::Program> {
    let mut ir = generate_ir(program)?;
    tracing::info!("IR Program before optimize:\n{:?}", ir);
    optimize::optimize(&mut ir);
    tracing::info!("IR Program:\n{:?}", ir);
    Ok(generate_mips_from_ir(ir)?)
}

pub fn generate_ir(program: ayysee_parser::ast::Program) -> anyhow::Result<Program> {
    let mut state = State::default();
    let block = state.new_block(true);
    state.init(block);

    process_stmts(&mut state, block, &program.statements)?;

    Ok(state.program)
}

fn process_stmts(
    state: &mut State,
    mut block: BlockId,
    statements: &[ast::Statement],
) -> anyhow::Result<BlockId> {
    for stmt in statements {
        tracing::debug!("{:?}", stmt);
        match stmt {
            ast::Statement::FunctionCall {
                identifier,
                arguments,
            } => {
                let args: Vec<VarOrConst> = arguments
                    .iter()
                    .map(|a| process_expr(state, block, a))
                    .collect();
                state.add_variable(
                    block,
                    VarValue::Call {
                        name: identifier.to_string(),
                        args,
                    },
                );
            }
            ast::Statement::Definition {
                identifier,
                expression,
            } => {
                let v = process_expr(state, block, &expression);
                let id = match v {
                    VarOrConst::Const(_) => state.add_variable(block, VarValue::Single(v)),
                    VarOrConst::Var(id) => id,
                    VarOrConst::External(_) => state.add_variable(block, VarValue::Single(v)),
                };
                state.assign(block, identifier.as_ref(), id);
            }
            ast::Statement::Assignment {
                identifier,
                expression,
            } => {
                let v = process_expr(state, block, &expression);
                let id = match v {
                    VarOrConst::Const(_) => state.add_variable(block, VarValue::Single(v)),
                    VarOrConst::Var(id) => id,
                    VarOrConst::External(_) => state.add_variable(block, VarValue::Single(v)),
                };
                state.assign(block, identifier.as_ref(), id);
            }
            ast::Statement::IfStatement(if_stmt) => match if_stmt {
                ast::IfStatement::If {
                    condition: _,
                    body: _,
                } => todo!(),
                ast::IfStatement::IfElse {
                    condition,
                    body,
                    else_body,
                } => {
                    let sealed = state.sealed_blocks.contains(&block);
                    let cond_id = process_expr(state, block, condition);

                    let block_body = state.new_block(sealed);
                    state.connect_blocks(block, block_body);
                    let block_body_end = process_stmts(state, block_body, body.statements())?;

                    let block_else = state.new_block(sealed);
                    state.connect_blocks(block, block_else);
                    let else_body_end = process_stmts(state, block_else, else_body.statements())?;

                    state.program.blocks[block.0]
                        .instructions
                        .push(Instruction::Branch {
                            cond: cond_id,
                            true_block: block_body,
                            false_block: block_else,
                        });
                    block = state.new_block(sealed);
                    state.connect_blocks(block_body_end, block);
                    state.connect_blocks(else_body_end, block);
                }
            },
            ast::Statement::Loop { body } => {
                // Prepare the next block, so that break statements can move to it
                let block_next = state.new_block(false);
                let block_body = state.new_block(false);

                state.connect_blocks(block, block_body);

                let body_end = process_stmts(state, block_body, body.statements())?;

                state.connect_blocks(body_end, block_body);
                if state.sealed_blocks.contains(&block) {
                    state.seal_block(block_body);
                }

                block = block_next;
            }
            ast::Statement::Yield {} => {
                state.program.blocks[block.0]
                    .instructions
                    .push(Instruction::Yield);
            }
            _ => {
                anyhow::bail!("unimplemented statement {:?}", stmt);
            }
        }
    }
    Ok(block)
}

fn process_expr(state: &mut State, block: BlockId, expr: &ayysee_parser::ast::Expr) -> VarOrConst {
    match expr {
        ayysee_parser::ast::Expr::Constant(v) => VarOrConst::Const(Into::<f64>::into(v).into()),
        ayysee_parser::ast::Expr::Identifier(ident) => {
            VarOrConst::Var(state.read_variable(block, ident.as_ref()))
        }
        ayysee_parser::ast::Expr::BinaryOp(lhs_expr, op, rhs_expr) => {
            let lhs = process_expr(state, block, lhs_expr);
            let rhs = process_expr(state, block, rhs_expr);
            VarOrConst::Var(state.add_variable(block, VarValue::BinaryOp { lhs, op: *op, rhs }))
        }
        ayysee_parser::ast::Expr::UnaryOp(_, _) => todo!(),
        ayysee_parser::ast::Expr::FunctionCall(ident, args) => {
            let args = args.iter().map(|a| process_expr(state, block, a)).collect();
            VarOrConst::Var(state.add_variable(
                block,
                VarValue::Call {
                    name: ident.to_string(),
                    args,
                },
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulator::{Simulator, TickResult};
    use ayysee_parser::grammar::ProgramParser;
    use stationeers_mips::types::{Device, DeviceVariable};
    use test_log::test;

    fn compile(ayysee: &str) -> mips::Program {
        let parser = ProgramParser::new();
        let ayysee_program = parser.parse(ayysee).unwrap();
        tracing::debug!("ayysee_program:\n{:?}", ayysee_program);
        let mips = generate_program(ayysee_program).unwrap();
        tracing::debug!("MIPS:\n{}", mips);
        mips
    }

    #[test]
    #[ignore]
    fn test_empty_program() {
        let mips = compile("");
        let mut simulator = Simulator::new(mips);
        assert_eq!(simulator.tick(), TickResult::End);
    }

    #[test]
    fn test_simple_store() {
        let mips = compile(
            r"
                // comments are supported
                store(d0, Setting, 1);
            ",
        );
        let mut simulator = Simulator::new(mips);
        assert_eq!(simulator.tick(), TickResult::End);
        assert_eq!(simulator.read(Device::D0, DeviceVariable::Setting), 1.0);
    }

    #[test]
    fn test_simple_variable() {
        let mips = compile(
            r"
                let x = 1;
                let y = x + 2;
                store(d0, Setting, y);
            ",
        );
        let mut simulator = Simulator::new(mips);
        assert_eq!(simulator.tick(), TickResult::End);
        assert_eq!(simulator.read(Device::D0, DeviceVariable::Setting), 3.0);
    }

    #[test]
    fn test_simple_load() {
        let mips = compile(
            r"
                let x = load(d0, Setting);
                let y = x + 2;
                store(d0, Setting, y);
                ",
        );
        let mut simulator = Simulator::new(mips);
        simulator.write(Device::D0, DeviceVariable::Setting, 2.0);
        assert_eq!(simulator.tick(), crate::simulator::TickResult::End);
        assert_eq!(simulator.read(Device::D0, DeviceVariable::Setting), 4.0);
    }

    #[test]
    fn test_simple_conditional() {
        let mips = compile(
            r"
                if load(d0, Setting) > 5 {
                    store(d0, Setting, 1);
                } else {
                    store(d0, Setting, 2);
                }
            ",
        );
        {
            let mut simulator = Simulator::new(mips.clone());
            simulator.write(Device::D0, DeviceVariable::Setting, 2.0);
            assert_eq!(simulator.tick(), crate::simulator::TickResult::End);
            assert_eq!(simulator.read(Device::D0, DeviceVariable::Setting), 2.0);
        }
        {
            let mut simulator = Simulator::new(mips);
            simulator.write(Device::D0, DeviceVariable::Setting, 8.0);
            assert_eq!(simulator.tick(), crate::simulator::TickResult::End);
            assert_eq!(simulator.read(Device::D0, DeviceVariable::Setting), 1.0);
        }
    }

    #[test]
    fn test_assignment_in_conditional() {
        let mips = compile(
            r"
                let x = 0;
                if load(d0, Setting) > 5 {
                    x = 1;
                } else {
                    x = 2;
                }
                store(d0, Setting, x);
            ",
        );
        {
            let mut simulator = Simulator::new(mips.clone());
            simulator.write(Device::D0, DeviceVariable::Setting, 2.0);
            assert_eq!(simulator.tick(), crate::simulator::TickResult::End);
            assert_eq!(simulator.read(Device::D0, DeviceVariable::Setting), 2.0);
        }
        {
            let mut simulator = Simulator::new(mips);
            simulator.write(Device::D0, DeviceVariable::Setting, 8.0);
            assert_eq!(simulator.tick(), crate::simulator::TickResult::End);
            assert_eq!(simulator.read(Device::D0, DeviceVariable::Setting), 1.0);
        }
    }

    #[test]
    fn test_loop() {
        let mips = compile(
            r"
                let x = 0;
                loop {
                    x = x + 1;
                    store(d0, Setting, x);
                    yield;
                }
            ",
        );
        let mut simulator = Simulator::new(mips.clone());
        assert_eq!(simulator.tick(), crate::simulator::TickResult::Yield);
        assert_eq!(simulator.read(Device::D0, DeviceVariable::Setting), 1.0);
        assert_eq!(simulator.tick(), crate::simulator::TickResult::Yield);
        assert_eq!(simulator.read(Device::D0, DeviceVariable::Setting), 2.0);
    }

    #[test]
    fn test_web_example() {
        let mips = compile(
            r"
// Welcome to the Ayysee Compiler!

// Example code
const tank = d0;
const regulator = d1;

loop {
    let pressure = load(tank, Pressure);
    store(regulator, Setting, pressure);
}
            ",
        );
        let mut simulator = Simulator::new(mips.clone());
        assert_eq!(simulator.tick(), crate::simulator::TickResult::Yield);
        assert_eq!(simulator.read(Device::D0, DeviceVariable::Setting), 1.0);
    }
}
