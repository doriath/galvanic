use super::types::{BlockId, VarId, VarOrConst, VarValue};
use crate::ir;
use crate::ir::register_allocation::RegisterAllocation;
use ayysee_parser::ast;
use mips::types::{Register, RegisterOrNumber};
use stationeers_mips as mips;
use std::collections::HashMap;

struct State<'a> {
    mips_program: mips::instructions::Program,
    ir_program: &'a ir::Program,
    registers: RegisterAllocation,
    block_start: HashMap<BlockId, usize>,
    // The location of jumps that want to jump to the end
    jump_to_end: Vec<usize>,
}

impl<'a> State<'a> {
    pub fn new(ir_program: &'a ir::Program) -> anyhow::Result<Self> {
        let registers = RegisterAllocation::allocate(&ir_program)?;
        Ok(Self {
            mips_program: Default::default(),
            ir_program,
            registers,
            block_start: Default::default(),
            jump_to_end: Default::default(),
        })
    }

    fn var_to_register(&self, v: &VarOrConst) -> RegisterOrNumber {
        match v {
            VarOrConst::Var(id) => RegisterOrNumber::Register(self.registers.get(*id).unwrap()),
            VarOrConst::External(_) => {
                panic!(
                    "not possible to convert external {:?} to RegisterOrNumber",
                    v
                )
            }
            VarOrConst::Const(x) => RegisterOrNumber::Number((*x).into()),
        }
    }

    fn generate_block(&mut self, block_id: BlockId) -> anyhow::Result<()> {
        // If block is already generated, just jump to it
        if let Some(pos) = self.block_start.get(&block_id) {
            self.mips_program.instructions.push(
                mips::instructions::FlowControl::Jump {
                    a: (*pos as f64).into(),
                }
                .into(),
            );
            return Ok(());
        }

        self.block_start
            .insert(block_id, self.mips_program.instructions.len());
        let block = &self.ir_program.blocks[block_id.0];
        for ins in &block.instructions {
            match ins {
                ir::Instruction::Assignment { id, value } => self.generate_assignment(id, value)?,
                ir::Instruction::Branch {
                    cond,
                    true_block,
                    false_block,
                } => {
                    self.generate_branch(cond, true_block, false_block)?;
                    return Ok(());
                }
                ir::Instruction::Yield => {
                    self.mips_program
                        .instructions
                        .push(mips::instructions::Instruction::new_yield());
                }
                ir::Instruction::Return(_) => {
                    self.mips_program.instructions.push(
                        mips::instructions::FlowControl::Jump {
                            a: Register::Ra.into(),
                        }
                        .into(),
                    );
                }
            }
        }
        anyhow::ensure!(block.next.len() < 2);
        for next in &block.next {
            self.generate_block(*next)?;
        }
        if block.next.is_empty() {
            self.jump_to_end.push(self.mips_program.instructions.len());
            self.mips_program
                .instructions
                .push(mips::instructions::FlowControl::Jump { a: (-1.0).into() }.into());
        }
        Ok(())
    }

    fn generate_assignment(&mut self, id: &VarId, value: &VarValue) -> anyhow::Result<()> {
        let register = self.registers.get(*id).unwrap();
        match value {
            VarValue::Single(simple) => self.mips_program.instructions.push(
                mips::instructions::Misc::Move {
                    register,
                    a: self.var_to_register(simple),
                }
                .into(),
            ),
            VarValue::BinaryOp { lhs, op, rhs } => {
                let a = self.var_to_register(lhs);
                let b = self.var_to_register(rhs);
                let instruction = match op {
                    ast::BinaryOpcode::Add => {
                        mips::instructions::Arithmetic::Add { register, a, b }.into()
                    }
                    ast::BinaryOpcode::Sub => {
                        mips::instructions::Arithmetic::Subtract { register, a, b }.into()
                    }
                    ast::BinaryOpcode::Mul => {
                        mips::instructions::Arithmetic::Multiply { register, a, b }.into()
                    }
                    ast::BinaryOpcode::Div => {
                        mips::instructions::Arithmetic::Divide { register, a, b }.into()
                    }
                    ast::BinaryOpcode::Conj => {
                        mips::instructions::Logic::And { register, a, b }.into()
                    }
                    ast::BinaryOpcode::Disj => {
                        mips::instructions::Logic::Or { register, a, b }.into()
                    }
                    ast::BinaryOpcode::Equals => {
                        mips::instructions::VariableSelection::SelectEqual { register, a, b }.into()
                    }
                    ast::BinaryOpcode::NotEquals => {
                        mips::instructions::VariableSelection::SelectNotEqual { register, a, b }
                            .into()
                    }
                    ast::BinaryOpcode::Greater => {
                        mips::instructions::VariableSelection::SelectGreaterThan { register, a, b }
                            .into()
                    }
                    ast::BinaryOpcode::GreaterEquals => {
                        mips::instructions::VariableSelection::SelectGreaterOrEqual {
                            register,
                            a,
                            b,
                        }
                        .into()
                    }
                    ast::BinaryOpcode::Lower => {
                        mips::instructions::VariableSelection::SelectLessThan { register, a, b }
                            .into()
                    }
                    ast::BinaryOpcode::LowerEquals => {
                        mips::instructions::VariableSelection::SelectLessOrEqual { register, a, b }
                            .into()
                    }
                };
                self.mips_program.instructions.push(instruction);
            }
            VarValue::Call { name, args } => {
                if name == "store" {
                    self.mips_program.instructions.push(
                        mips::instructions::DeviceIo::StoreDeviceVariable {
                            device: args[0].external().unwrap().parse().unwrap(),
                            variable: args[1].external().unwrap().parse().unwrap(),
                            register: self.var_to_register(&args[2]),
                        }
                        .into(),
                    );
                } else if name == "load" {
                    self.mips_program.instructions.push(
                        mips::instructions::DeviceIo::LoadDeviceVariable {
                            register,
                            device: args[0].external().unwrap().parse().unwrap(),
                            variable: args[1].external().unwrap().parse().unwrap(),
                        }
                        .into(),
                    )
                } else {
                    let f = match self.ir_program.functions.get(name) {
                        None => anyhow::bail!("function {} not found", name),
                        Some(x) => x,
                    };
                    // This has to be fixed later.
                    self.mips_program.instructions.push(
                        mips::instructions::FlowControl::Jump {
                            a: (f.block_id.0 as f64).into(),
                        }
                        .into(),
                    );
                    // self.generate_block(f.block_id)?;
                }
            }
            VarValue::Phi(_) => (),
            VarValue::Param => (),
        }
        Ok(())
    }

    fn generate_branch(
        &mut self,
        cond_var: &VarOrConst,
        true_block_id: &BlockId,
        false_block_id: &BlockId,
    ) -> anyhow::Result<()> {
        // record the index of current instruction, so that we can edit it later
        let jeqz_idx = self.mips_program.instructions.len();
        self.mips_program.instructions.push(
            mips::instructions::FlowControl::BranchEqualZero {
                a: self.var_to_register(cond_var),
                b: (-1.0).into(),
            }
            .into(),
        );

        self.generate_block(*true_block_id)?;
        self.generate_block(*false_block_id)?;

        // Fix branch jump
        let idx = self.block_start[false_block_id];
        self.mips_program.instructions[jeqz_idx] =
            mips::instructions::FlowControl::BranchEqualZero {
                a: self.var_to_register(cond_var),
                b: RegisterOrNumber::Number(idx as f64),
            }
            .into();
        Ok(())
    }
}

// The Program is expected to be in SSA form (each variable assigned once)
pub fn generate_mips_from_ir(
    ir_program: ir::Program,
) -> anyhow::Result<mips::instructions::Program> {
    let mut state = State::new(&ir_program)?;
    state.generate_block(BlockId(0))?;
    for i in state.jump_to_end {
        state.mips_program.instructions[i] = mips::instructions::FlowControl::Jump {
            a: (state.mips_program.instructions.len() as f64).into(),
        }
        .into();
    }

    Ok(state.mips_program)
}
