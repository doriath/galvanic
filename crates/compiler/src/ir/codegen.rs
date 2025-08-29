use std::collections::HashMap;

use ayysee_parser::ast;
use mips::types::{Number, Register, RegisterOrNumber};
use stationeers_mips as mips;

use super::types::{BlockId, VarId, VarOrConst};

pub fn generate_mips_from_ir(
    program: &crate::ir::Program,
) -> anyhow::Result<mips::instructions::Program> {
    let mut result = mips::instructions::Program::default();

    let mut registers = HashMap::<VarId, Register>::default();
    let var_to_register =
        |v: &VarOrConst, registers: &HashMap<VarId, Register>| -> RegisterOrNumber {
            match v {
                VarOrConst::Var(id) => RegisterOrNumber::Register(*registers.get(id).unwrap()),
                VarOrConst::External(_) => {
                    panic!(
                        "not possible to convert external {:?} to RegisterOrNumber",
                        v
                    )
                }
                VarOrConst::Const(x) => RegisterOrNumber::Number(Number::Float(*x as f32)),
            }
        };

    for block in &program.blocks {
        for ins in &block.instructions {
            match ins {
                super::types::Instruction::Assignment { id, value } => match value {
                    super::types::VarValue::Single(_) => todo!(),
                    // TODO
                    super::types::VarValue::BinaryOp { lhs, op, rhs } => match op {
                        ast::BinaryOpcode::Add => {
                            let a = var_to_register(lhs, &registers);
                            let b = var_to_register(rhs, &registers);
                            result.instructions.push(
                                mips::instructions::Arithmetic::Add {
                                    register: Register::R0,
                                    a,
                                    b,
                                }
                                .into(),
                            );
                            registers.insert(*id, Register::R0);
                        }
                        ast::BinaryOpcode::Greater => {
                            let a = var_to_register(lhs, &registers);
                            let b = var_to_register(rhs, &registers);
                            result.instructions.push(
                                mips::instructions::VariableSelection::SelectGreaterThan {
                                    register: Register::R0,
                                    a,
                                    b,
                                }
                                .into(),
                            );
                            registers.insert(*id, Register::R0);
                        }
                        _ => todo!(),
                    },
                    super::types::VarValue::Call { name, args } => {
                        if name == "store" {
                            result.instructions.push(
                                mips::instructions::DeviceIo::StoreDeviceVariable {
                                    // TODO
                                    device: mips::types::Device::D0,
                                    // TODO
                                    variable: mips::types::DeviceVariable::Setting,
                                    // TODO
                                    register: var_to_register(&args[2], &registers),
                                }
                                .into(),
                            );
                        } else if name == "load" {
                            registers.insert(*id, Register::R0);
                            result.instructions.push(
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
                    // TODO: not implemented yet
                    result
                        .instructions
                        .push(mips::instructions::FlowControl::Jump { a: 0 }.into());
                }
            }
        }
    }
    Ok(result)
}
