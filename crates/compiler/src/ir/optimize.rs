use std::collections::{HashMap, HashSet};

use crate::ir::types::{Block, Instruction, Program, VarId, VarOrConst, VarValue};

use super::types::BlockId;

pub fn optimize(program: &mut Program) {
    remove_unused_variables(program)
}

// Returns true if any variables were removed.
fn remove_unused_variables(program: &mut Program) {
    let mut pos = HashMap::<VarId, (BlockId, usize)>::default();
    let mut stack: Vec<VarId> = Vec::default();
    let mut used = HashSet::<VarId>::default();
    for (block_id, block) in program.blocks.iter().enumerate() {
        for (ins_id, ins) in block.instructions.iter().enumerate() {
            if let Instruction::Assignment { id, value } = ins {
                pos.insert(*id, (BlockId(block_id), ins_id));
                if let VarValue::Call { name, args } = value {
                    if name == "store" {
                        used.insert(*id);
                        stack.push(*id);
                        for arg in args {
                            if let VarOrConst::Var(id) = arg {
                                used.insert(*id);
                                stack.push(*id);
                            }
                        }
                    }
                }
            }
        }
    }
    while !stack.is_empty() {
        let id = stack.pop().unwrap();
        used.insert(id);
        let p = pos.get(&id).unwrap();
        let ins = &program.blocks[p.0 .0].instructions[p.1];
        let mut maybe_add = |v: &VarOrConst| {
            if let VarOrConst::Var(x) = v {
                if !used.contains(x) {
                    used.insert(*x);
                    stack.push(*x);
                }
            }
        };
        if let Instruction::Assignment { id, value } = ins {
            match value {
                VarValue::Single(x) => maybe_add(x),
                VarValue::BinaryOp { lhs, op, rhs } => {
                    maybe_add(lhs);
                    maybe_add(rhs);
                }
                VarValue::Call { name, args } => {
                    for a in args {
                        maybe_add(a);
                    }
                }
            }
        }
    }
    for b in &mut program.blocks {
        b.instructions.retain(|x| match &x {
            Instruction::Assignment { id, value } => used.contains(id),
            _ => true,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;

    #[test]
    fn test_remove_unused_variables() {
        let mut program = Program {
            blocks: vec![Block {
                instructions: vec![Instruction::Assignment {
                    id: VarId(0),
                    value: VarValue::Single(VarOrConst::Const(1.0)),
                }],
                next: vec![],
                prev: vec![],
            }],
        };
        optimize(&mut program);
        assert_eq!(program.blocks[0].instructions.len(), 0);
    }
}
