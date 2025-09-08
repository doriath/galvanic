mod codegen;
mod optimize;
mod register_allocation;
pub mod types;

use crate::ir::codegen::generate_mips_from_ir;
use anyhow::Context;
use ayysee_parser::ast::{self, Expr};
use stationeers_mips as mips;
use std::collections::{HashMap, HashSet};
pub use types::*;

struct State {
    defs: HashMap<String, HashMap<BlockId, VarId>>,
    consts: HashMap<String, VarOrConst>,
    next_var: VarId,
    program: Program,
    sealed_blocks: HashSet<BlockId>,
    unresolved_phis: HashMap<BlockId, Vec<(String, VarId, usize)>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            defs: Default::default(),
            consts: Default::default(),
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
        if let Some(x) = self
            .defs
            .get(name)
            .context(format!("{}", name))
            .unwrap()
            .get(&block)
        {
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
        let externals = vec![
            "db",
            "d0",
            "d1",
            "d2",
            "d3",
            "d4",
            "d5",
            "Activate",
            "AirRelease",
            "Charge",
            "ClearMemory",
            "Color",
            "CompletionRatio",
            "ElevatorLevel",
            "ElevatorSpeed",
            "Error",
            "ExportCount",
            "Filtration",
            "Harvest",
            "Horizontal",
            "HorizontalRatio",
            "Idle",
            "ImportCount",
            "Lock",
            "Maximum",
            "Mode",
            "On",
            "Open",
            "Output",
            "Plant",
            "PositionX",
            "PositionY",
            "Power",
            "PowerActual",
            "PowerPotential",
            "PowerRequired",
            "Pressure",
            "PressureExternal",
            "PressureInternal",
            "PressureSetting",
            "Quantity",
            "Ratio",
            "RatioCarbonDioxide",
            "RatioNitrogen",
            "RatioOxygen",
            "RatioPollutant",
            "RatioVolatiles",
            "RatioWater",
            "Reagents",
            "RecipeHash",
            "RequestHash",
            "RequiredPower",
            "Setting",
            "SolarAngle",
            "Temperature",
            "TemperatureSettings",
            "TotalMoles",
            "VelocityMagnitude",
            "VelocityRelativeX",
            "VelocityRelativeY",
            "VelocityRelativeZ",
            "Vertical",
            "VerticalRatio",
            "Volume",
        ];

        for external in externals {
            self.consts
                .insert(external.into(), VarOrConst::External(external.into()));
        }
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
            ast::Statement::Assignment { lhs, rhs } => {
                let v = process_expr(state, block, &rhs);
                let id = match v {
                    VarOrConst::Var(id) => id,
                    _ => state.add_variable(block, v.into()),
                };
                match *(*lhs) {
                    ast::Expr::Identifier(ref ident) => state.assign(block, ident.as_ref(), id),
                    ast::Expr::FieldExpr(ref d, ref logic) => {
                        let arg0 = process_expr(state, block, &Expr::Identifier(d.clone()));
                        let arg1 = process_expr(state, block, &Expr::Identifier(logic.clone()));
                        state.add_variable(
                            block,
                            VarValue::Call {
                                name: "store".to_string(),
                                args: vec![arg0, arg1, id.into()],
                            },
                        );
                    }
                    _ => anyhow::bail!(
                        "unsupported assignment, left side is expected to be identifier"
                    ),
                }
            }
            ast::Statement::Constant(identifier, expression) => {
                let v = process_expr(state, block, &expression);
                state.consts.insert(identifier.to_string(), v);
            }
            ast::Statement::IfStatement(if_stmt) => match if_stmt {
                ast::IfStatement::If { condition, body } => {
                    process_cond(
                        state,
                        &mut block,
                        condition,
                        body,
                        &ast::Block::Statements(vec![]),
                    )?;
                }
                ast::IfStatement::IfElse {
                    condition,
                    body,
                    else_body,
                } => {
                    process_cond(state, &mut block, condition, body, else_body)?;
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

fn process_cond(
    state: &mut State,
    block_id: &mut BlockId,
    cond_expr: &Expr,
    true_block: &ast::Block,
    false_block: &ast::Block,
) -> anyhow::Result<()> {
    let sealed = state.sealed_blocks.contains(&block_id);
    let cond_var = process_expr(state, *block_id, cond_expr);

    let true_block_id_start = state.new_block(sealed);
    state.connect_blocks(*block_id, true_block_id_start);
    let true_block_id_end = process_stmts(state, true_block_id_start, true_block.statements())?;

    let false_block_id_start = state.new_block(sealed);
    state.connect_blocks(*block_id, false_block_id_start);
    let false_block_id_end = process_stmts(state, false_block_id_start, false_block.statements())?;

    state.program.blocks[block_id.0]
        .instructions
        .push(Instruction::Branch {
            cond: cond_var,
            true_block: true_block_id_start,
            false_block: false_block_id_start,
        });
    *block_id = state.new_block(sealed);
    state.connect_blocks(true_block_id_end, *block_id);
    state.connect_blocks(false_block_id_end, *block_id);
    Ok(())
}

fn process_expr(state: &mut State, block: BlockId, expr: &ayysee_parser::ast::Expr) -> VarOrConst {
    match expr {
        Expr::Constant(v) => VarOrConst::Const(Into::<f64>::into(v).into()),
        Expr::Identifier(ident) => {
            if let Some(x) = state.consts.get(&ident.to_string()) {
                x.clone()
            } else {
                VarOrConst::Var(state.read_variable(block, ident.as_ref()))
            }
        }
        Expr::BinaryOp(lhs_expr, op, rhs_expr) => {
            let lhs = process_expr(state, block, lhs_expr);
            let rhs = process_expr(state, block, rhs_expr);
            VarOrConst::Var(state.add_variable(block, VarValue::BinaryOp { lhs, op: *op, rhs }))
        }
        Expr::UnaryOp(_, _) => todo!(),
        Expr::FunctionCall(ident, args) => {
            let args = args.iter().map(|a| process_expr(state, block, a)).collect();
            VarOrConst::Var(state.add_variable(
                block,
                VarValue::Call {
                    name: ident.to_string(),
                    args,
                },
            ))
        }
        Expr::FieldExpr(d, logic) => {
            let arg0 = process_expr(state, block, &Expr::Identifier(d.clone()));
            let arg1 = process_expr(state, block, &Expr::Identifier(logic.clone()));

            VarOrConst::Var(state.add_variable(
                block,
                VarValue::Call {
                    name: "load".to_string(),
                    args: vec![arg0, arg1],
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
    fn test_new_store_syntax() {
        let mips = compile(
            r"
                d0.Setting = 1;
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
    fn test_new_load() {
        let mips = compile(
            r"
                store(d0, Setting, d0.Setting + 2);
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
    fn test_conditional_without_else() {
        let mips = compile(
            r"
                let x = 1;
                if d0.Setting > 5 {
                    let x = 2;
                }
                d1.Setting = x;
            ",
        );
        {
            let mut simulator = Simulator::new(mips.clone());
            simulator.write(Device::D0, DeviceVariable::Setting, 3.0);
            assert_eq!(simulator.tick(), crate::simulator::TickResult::End);
            assert_eq!(simulator.read(Device::D1, DeviceVariable::Setting), 1.0);
        }
        {
            let mut simulator = Simulator::new(mips);
            simulator.write(Device::D0, DeviceVariable::Setting, 8.0);
            assert_eq!(simulator.tick(), crate::simulator::TickResult::End);
            assert_eq!(simulator.read(Device::D1, DeviceVariable::Setting), 2.0);
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
// Welcome to the ayysee Compiler!

// Example code
const base = db;
const gas_sensor = d1;

loop {
    let temp = gas_sensor.Temperature;
    if temp > 200 {
        base.Setting = 0;
    } else {
        base.Setting = 1;
    }
    yield;
}
            ",
        );
        let mut simulator = Simulator::new(mips.clone());
        assert_eq!(simulator.tick(), crate::simulator::TickResult::Yield);
    }

    #[test]
    fn test_supports_comparators() {
        let mips = compile(
            r"
                let x = d0.Temperature * d0.Pressure + d0.Setting / d0.On;
                if x > 0 || d0.Temperature >= 0 && d0.Power < 0 || d0.Output <= 0 || d0.Lock == 0 || d0.Mode != 1 {
                    let x = 2;
                }
                d1.Setting = x;
            ",
        );
        let mut simulator = Simulator::new(mips);
        assert_eq!(simulator.tick(), crate::simulator::TickResult::End);
        // This is just a sanity check that we can process all those operations
    }
}
