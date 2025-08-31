use std::collections::HashMap;

use ayysee_parser::ast;
use mips::types::{Number, Register, RegisterOrNumber};
use stationeers_mips as mips;

use super::types::{BlockId, VarId, VarOrConst};

#[derive(Default)]
struct State {
    registers: HashMap<VarId, Register>,
    program: mips::instructions::Program,
}

impl State {
    fn var_to_register(&self, v: &VarOrConst) -> RegisterOrNumber {
        match v {
            VarOrConst::Var(id) => RegisterOrNumber::Register(*self.registers.get(id).unwrap()),
            VarOrConst::External(_) => {
                panic!(
                    "not possible to convert external {:?} to RegisterOrNumber",
                    v
                )
            }
            VarOrConst::Const(x) => RegisterOrNumber::Number(Number::Float(*x as f32)),
        }
    }
}

pub fn generate_mips_from_ir(
    program: &crate::ir::Program,
) -> anyhow::Result<mips::instructions::Program> {
    let mut state = State::default();

    generate_block(&mut state, BlockId(0), program);

    Ok(state.program)
}

fn generate_block(state: &mut State, block_id: BlockId, ir_program: &crate::ir::Program) {
    let block = &ir_program.blocks[block_id.0];
    for ins in &block.instructions {
        match ins {
            super::types::Instruction::Assignment { id, value } => match value {
                super::types::VarValue::Single(_) => todo!(),
                // TODO
                super::types::VarValue::BinaryOp { lhs, op, rhs } => match op {
                    ast::BinaryOpcode::Add => {
                        let a = state.var_to_register(lhs);
                        let b = state.var_to_register(rhs);
                        state.program.instructions.push(
                            mips::instructions::Arithmetic::Add {
                                register: Register::R0,
                                a,
                                b,
                            }
                            .into(),
                        );
                        state.registers.insert(*id, Register::R0);
                    }
                    ast::BinaryOpcode::Greater => {
                        let a = state.var_to_register(lhs);
                        let b = state.var_to_register(rhs);
                        state.program.instructions.push(
                            mips::instructions::VariableSelection::SelectGreaterThan {
                                register: Register::R0,
                                a,
                                b,
                            }
                            .into(),
                        );
                        state.registers.insert(*id, Register::R0);
                    }
                    _ => todo!(),
                },
                super::types::VarValue::Call { name, args } => {
                    if name == "store" {
                        state.program.instructions.push(
                            mips::instructions::DeviceIo::StoreDeviceVariable {
                                // TODO
                                device: mips::types::Device::D0,
                                // TODO
                                variable: mips::types::DeviceVariable::Setting,
                                // TODO
                                register: state.var_to_register(&args[2]),
                            }
                            .into(),
                        );
                    } else if name == "load" {
                        state.registers.insert(*id, Register::R0);
                        state.program.instructions.push(
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
                super::types::VarValue::Phi(_) => todo!(),
            },
            super::types::Instruction::Branch {
                cond,
                true_block,
                false_block,
            } => {
                // record the index of current instruction, so that we can edit it later
                let jeqz_idx = state.program.instructions.len();
                state.program.instructions.push(
                    mips::instructions::FlowControl::BranchEqualZero {
                        a: state.var_to_register(cond),
                        b: RegisterOrNumber::Number(Number::Float(0.0)),
                    }
                    .into(),
                );

                generate_block(state, *true_block, ir_program);

                // Similar to above, record current index, to modify later
                let jr_idx = state.program.instructions.len();
                state
                    .program
                    .instructions
                    .push(mips::instructions::FlowControl::Jump { a: 0 }.into());

                generate_block(state, *false_block, ir_program);

                // Fix relative jumps
                state.program.instructions[jeqz_idx] =
                    mips::instructions::FlowControl::RelativeBranchEqualZero {
                        a: state.var_to_register(cond),
                        b: RegisterOrNumber::Number(Number::Int((jr_idx + 1) as i32)),
                    }
                    .into();
                state.program.instructions[jr_idx] =
                    mips::instructions::FlowControl::JumpRelative {
                        a: state.program.instructions.len() as i32,
                    }
                    .into();
            }
        }
    }
}
