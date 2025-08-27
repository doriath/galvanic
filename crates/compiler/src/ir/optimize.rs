use std::collections::{HashMap, HashSet};

use crate::ir::types::{Block, Instruction, Program, VarId, VarOrConst, VarValue};

use super::types::BlockId;

pub fn optimize(program: &mut Program) {
    inline(program);
    remove_unused_variables(program);
}

// Returns true if any variables were removed.
fn remove_unused_variables(program: &mut Program) -> bool {
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
        if let Instruction::Assignment { id: _, value } = ins {
            match value {
                VarValue::Single(x) => maybe_add(x),
                VarValue::BinaryOp { lhs, op: _, rhs } => {
                    maybe_add(lhs);
                    maybe_add(rhs);
                }
                VarValue::Call { name: _, args } => {
                    for a in args {
                        maybe_add(a);
                    }
                }
            }
        }
    }
    let mut removed_any = false;
    for b in &mut program.blocks {
        let s = b.instructions.len();
        b.instructions.retain(|x| match &x {
            Instruction::Assignment { id, value: _ } => used.contains(id),
            _ => true,
        });
        if s != b.instructions.len() {
            removed_any = true;
        }
    }
    removed_any
}

fn fully_resolve(inlinable: &mut HashMap<VarId, VarOrConst>, id: VarId) -> VarOrConst {
    let val = inlinable.get(&id).unwrap();
    if let VarOrConst::Var(next) = val {
        if inlinable.contains_key(next) {
            let new_val = fully_resolve(inlinable, id);
            inlinable.insert(id, new_val.clone());
            return new_val;
        }
    }
    val.clone()
}

fn resolve_inlinable(inlinable: &mut HashMap<VarId, VarOrConst>) {
    let keys: Vec<VarId> = inlinable.keys().into_iter().copied().collect();
    for id in keys {
        fully_resolve(inlinable, id);
    }
}

// Inlines the variables where possible
fn inline(program: &mut Program) {
    let mut inlinable = HashMap::<VarId, VarOrConst>::default();
    for b in &program.blocks {
        for ins in &b.instructions {
            if let Instruction::Assignment { id, value } = ins {
                if let VarValue::Single(x) = value {
                    inlinable.insert(*id, x.clone());
                }
            }
        }
    }
    resolve_inlinable(&mut inlinable);
    tracing::info!("inlinable: {:?}", inlinable);

    let maybe_replace = |var: &mut VarOrConst| {
        if let VarOrConst::Var(x) = var {
            let y = inlinable.get(&x);
            if let Option::Some(new_val) = y {
                *var = new_val.clone();
            }
        }
    };

    for b in &mut program.blocks {
        for ins in &mut b.instructions {
            match ins {
                Instruction::Assignment { id, value } => match value {
                    VarValue::Single(x) => maybe_replace(x),
                    VarValue::BinaryOp { lhs, op, rhs } => {
                        maybe_replace(lhs);
                        maybe_replace(rhs);
                    }
                    VarValue::Call { name, args } => {
                        for arg in args {
                            maybe_replace(arg);
                        }
                    }
                },
                // TODO:
                Instruction::Branch {
                    cond,
                    true_block,
                    false_block,
                } => (),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ayysee_parser::grammar::ProgramParser;
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

    #[test]
    fn test_inlines_variables() {
        let parser = ProgramParser::new();
        let parsed = parser
            .parse(
                r"
                let x = 1;
                let y = x;
                let z = y;
                store(d0, Setting, z);
                ",
            )
            .unwrap();
        let mut program = crate::ir::generate_ir(parsed).unwrap();
        optimize(&mut program);
        assert_eq!(
            program.blocks[0].instructions.len(),
            1,
            "instructions: {:?}",
            program
        );
    }
}
