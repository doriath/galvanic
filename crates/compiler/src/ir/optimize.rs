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
            match ins {
                Instruction::Assignment { id, value } => {
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
                Instruction::Branch {
                    cond,
                    true_block: _,
                    false_block: _,
                } => {
                    if let VarOrConst::Var(id) = cond {
                        used.insert(*id);
                        stack.push(*id);
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
                VarValue::Phi(phi) => {
                    for x in phi {
                        if !used.contains(x) {
                            used.insert(*x);
                            stack.push(*x);
                        }
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

#[derive(Default)]
struct InlineState {
    inlined: HashSet<VarId>,
}

impl InlineState {
    fn find_var(&self, var_id: VarId, program: &Program) -> (BlockId, usize) {
        // TODO: optimize this, we should record the location of everything
        for (block_id, block) in program.blocks.iter().enumerate() {
            for (idx, ins) in block.instructions.iter().enumerate() {
                if let Instruction::Assignment { id, value } = ins {
                    if var_id == *id {
                        return (BlockId(block_id), idx);
                    }
                }
            }
        }
        unreachable!()
    }

    fn inline_variable(self: &mut InlineState, id: VarId, program: &mut Program) {
        if state.inlined.contains(&id) {
            return;
        }
        state.inlined.insert(id);
        let get_value = |id: VarId, state: &InlineState, program: &Program| -> VarValue {
            let (block_id, idx) = state.find_var(id, program);
            if let Instruction::Assignment { id: _, value } =
                &program.blocks[block_id.0].instructions[idx]
            {
                return value.clone();
            }
            unreachable!()
        };
        let inline_simple =
            |v: &VarOrConst, state: &mut InlineState, program: &mut Program| -> VarOrConst {
                match v {
                    VarOrConst::Var(id) => {
                        inline_variable(*id, state, program);
                        let next_value = get_value(*id, state, program);
                        if let VarValue::Single(s) = next_value {
                            return s.clone();
                        }
                    }
                    _ => (),
                }
                v.clone()
            };
        let (block_id, idx) = state.find_var(id, program);
        let value = get_value(id, state, program);
        match value {
            VarValue::Single(simple) => {
                let val = inline_simple(&simple, state, program);
                program.blocks[block_id.0].instructions[idx] = Instruction::Assignment {
                    id,
                    value: VarValue::Single(val),
                };
            }
            VarValue::Phi(vars) => {
                let new_vars = vars
                    .iter()
                    .map(|v| inline_simple(&VarOrConst::Var(*v), state, program))
                    .collect::<HashSet<_>>();
                if new_vars.len() == 1 {
                    program.blocks[block_id.0].instructions[idx] = Instruction::Assignment {
                        id,
                        value: VarValue::Single(new_vars.into_iter().next().unwrap()),
                    };
                }
            }
            VarValue::BinaryOp { lhs, op, rhs } => {
                let lhs = inline_simple(&lhs, state, program);
                let rhs = inline_simple(&rhs, state, program);
                program.blocks[block_id.0].instructions[idx] = Instruction::Assignment {
                    id,
                    value: VarValue::BinaryOp { lhs, op, rhs },
                };
            }
            VarValue::Call { name, args } => {
                let new_args: Vec<VarOrConst> = args
                    .iter()
                    .map(|a| inline_simple(a, state, program))
                    .collect();
                program.blocks[block_id.0].instructions[idx] = Instruction::Assignment {
                    id,
                    value: VarValue::Call {
                        name,
                        args: new_args,
                    },
                };
            }
        }
    }
}

// Inlines the variables where possible
fn inline(program: &mut Program) {
    let mut state = InlineState::default();
    let mut vars = HashSet::<VarId>::default();
    for b in &program.blocks {
        for ins in &b.instructions {
            if let Instruction::Assignment { id, value: _ } = ins {
                vars.insert(*id);
            }
        }
    }
    for id in vars {
        inline_variable(id, &mut state, program);
    }
    return;
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
                    value: VarValue::Single(VarOrConst::Const((1.0).into())),
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
