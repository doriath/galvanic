use std::collections::{HashMap, HashSet};

use crate::ir::types::{Instruction, Program, VarId, VarOrConst, VarValue};

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
                Instruction::Yield => (),
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

struct InlineState<'a> {
    program: &'a mut Program,
    inlined: HashSet<VarId>,
}

impl<'a> InlineState<'a> {
    pub fn inline_variable(&mut self, id: VarId) {
        if self.inlined.contains(&id) {
            return;
        }
        self.inlined.insert(id);
        match self.get_value(id) {
            VarValue::Single(simple) => {
                let val = self.inline_simple(&simple);
                self.set_var(id, val.into());
            }
            VarValue::Phi(vars) => {
                let new_vars = vars
                    .iter()
                    .map(|v| self.inline_simple(&(*v).into()))
                    .collect::<HashSet<_>>();
                if new_vars.len() == 1 {
                    self.set_var(id, new_vars.into_iter().next().unwrap().into())
                }
            }
            VarValue::BinaryOp { lhs, op, rhs } => {
                let lhs = self.inline_simple(&lhs);
                let rhs = self.inline_simple(&rhs);
                self.set_var(id, VarValue::BinaryOp { lhs, op, rhs });
            }
            VarValue::Call { name, args } => {
                let args: Vec<VarOrConst> = args.iter().map(|a| self.inline_simple(a)).collect();
                self.set_var(id, VarValue::Call { name, args });
            }
        }
    }

    fn find_var(&self, var_id: VarId) -> (BlockId, usize) {
        // TODO: optimize this, we should record the location of everything
        for (block_id, block) in self.program.blocks.iter().enumerate() {
            for (idx, ins) in block.instructions.iter().enumerate() {
                if let Instruction::Assignment { id, value: _ } = ins {
                    if var_id == *id {
                        return (BlockId(block_id), idx);
                    }
                }
            }
        }
        unreachable!("could not find {:?}", var_id)
    }

    fn get_value(&self, id: VarId) -> VarValue {
        let (block_id, idx) = self.find_var(id);
        if let Instruction::Assignment { id: _, value } =
            &self.program.blocks[block_id.0].instructions[idx]
        {
            return value.clone();
        }
        unreachable!()
    }

    fn inline_simple(&mut self, v: &VarOrConst) -> VarOrConst {
        match v {
            VarOrConst::Var(id) => {
                self.inline_variable(*id);
                let next_value = self.get_value(*id);
                if let VarValue::Single(s) = next_value {
                    return s.clone();
                }
            }
            _ => (),
        }
        v.clone()
    }

    fn set_var(&mut self, id: VarId, value: VarValue) {
        let (block_id, idx) = self.find_var(id);
        self.program.blocks[block_id.0].instructions[idx] = Instruction::Assignment { id, value };
    }
}

// Inlines the variables where possible
fn inline(program: &mut Program) {
    let mut vars = HashSet::<VarId>::default();
    for b in &program.blocks {
        for ins in &b.instructions {
            if let Instruction::Assignment { id, value: _ } = ins {
                vars.insert(*id);
            }
        }
    }
    let mut state = InlineState {
        program,
        inlined: HashSet::default(),
    };
    for id in vars {
        state.inline_variable(id);
    }
    return;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::types::Block;
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
