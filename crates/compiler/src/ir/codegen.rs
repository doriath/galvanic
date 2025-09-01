use super::types::{BlockId, VarId, VarOrConst, VarValue};
use crate::ir;
use ayysee_parser::ast;
use mips::types::{Number, Register, RegisterOrNumber};
use stationeers_mips as mips;
use std::collections::HashMap;

struct State<'a> {
    registers: HashMap<VarId, Register>,
    mips_program: mips::instructions::Program,
    ir_program: &'a ir::Program,
}

impl<'a> State<'a> {
    pub fn new(ir_program: &'a ir::Program) -> Self {
        Self {
            registers: Default::default(),
            mips_program: Default::default(),
            ir_program,
        }
    }

    fn var_to_register(&self, v: &VarOrConst) -> RegisterOrNumber {
        match v {
            VarOrConst::Var(id) => RegisterOrNumber::Register(*self.registers.get(id).unwrap()),
            VarOrConst::External(_) => {
                panic!(
                    "not possible to convert external {:?} to RegisterOrNumber",
                    v
                )
            }
            VarOrConst::Const(x) => {
                RegisterOrNumber::Number(Number::Float(Into::<f64>::into(*x) as f32))
            }
        }
    }

    fn generate_block(&mut self, block_id: BlockId) {
        let block = &self.ir_program.blocks[block_id.0];
        for ins in &block.instructions {
            match ins {
                ir::Instruction::Assignment { id, value } => self.generate_assignment(id, value),
                ir::Instruction::Branch {
                    cond,
                    true_block,
                    false_block,
                } => {
                    self.generate_branch(cond, true_block, false_block);
                }
                ir::Instruction::Yield => {
                    self.mips_program
                        .instructions
                        .push(mips::instructions::Instruction::new_yield());
                }
            }
        }
    }

    fn generate_assignment(&mut self, id: &VarId, value: &VarValue) {
        match value {
            VarValue::Single(_) => todo!(),
            // TODO
            VarValue::BinaryOp { lhs, op, rhs } => match op {
                ast::BinaryOpcode::Add => {
                    let a = self.var_to_register(lhs);
                    let b = self.var_to_register(rhs);
                    self.mips_program.instructions.push(
                        mips::instructions::Arithmetic::Add {
                            register: Register::R0,
                            a,
                            b,
                        }
                        .into(),
                    );
                    self.registers.insert(*id, Register::R0);
                }
                ast::BinaryOpcode::Greater => {
                    let a = self.var_to_register(lhs);
                    let b = self.var_to_register(rhs);
                    self.mips_program.instructions.push(
                        mips::instructions::VariableSelection::SelectGreaterThan {
                            register: Register::R0,
                            a,
                            b,
                        }
                        .into(),
                    );
                    self.registers.insert(*id, Register::R0);
                }
                _ => todo!(),
            },
            VarValue::Call { name, args } => {
                if name == "store" {
                    self.mips_program.instructions.push(
                        mips::instructions::DeviceIo::StoreDeviceVariable {
                            // TODO
                            device: mips::types::Device::D0,
                            // TODO
                            variable: mips::types::DeviceVariable::Setting,
                            // TODO
                            register: self.var_to_register(&args[2]),
                        }
                        .into(),
                    );
                } else if name == "load" {
                    self.registers.insert(*id, Register::R0);
                    self.mips_program.instructions.push(
                        mips::instructions::DeviceIo::LoadDeviceVariable {
                            register: mips::types::Register::R0,
                            device: mips::types::Device::D0,
                            variable: mips::types::DeviceVariable::Setting,
                        }
                        .into(),
                    )
                } else {
                    todo!()
                }
            }
            VarValue::Phi(_) => todo!(),
        }
    }

    fn generate_branch(&mut self, cond: &VarOrConst, true_block: &BlockId, false_block: &BlockId) {
        // record the index of current instruction, so that we can edit it later
        let jeqz_idx = self.mips_program.instructions.len();
        self.mips_program.instructions.push(
            mips::instructions::FlowControl::BranchEqualZero {
                a: self.var_to_register(cond),
                b: RegisterOrNumber::Number(Number::Float(0.0)),
            }
            .into(),
        );

        self.generate_block(*true_block);

        // Similar to above, record current index, to modify later
        let jr_idx = self.mips_program.instructions.len();
        self.mips_program
            .instructions
            .push(mips::instructions::FlowControl::Jump { a: 0 }.into());

        self.generate_block(*false_block);

        // Fix relative jumps
        self.mips_program.instructions[jeqz_idx] =
            mips::instructions::FlowControl::RelativeBranchEqualZero {
                a: self.var_to_register(cond),
                b: RegisterOrNumber::Number(Number::Int((jr_idx + 1) as i32)),
            }
            .into();
        self.mips_program.instructions[jr_idx] = mips::instructions::FlowControl::Jump {
            a: self.mips_program.instructions.len() as i32,
        }
        .into();
    }
}

pub fn generate_mips_from_ir(
    ir_program: &ir::Program,
) -> anyhow::Result<mips::instructions::Program> {
    let mut state = State::new(ir_program);
    state.generate_block(BlockId(0));
    Ok(state.mips_program)
}
