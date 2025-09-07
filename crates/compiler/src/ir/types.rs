use std::collections::HashSet;

use ayysee_parser::ast::BinaryOpcode;
use ordered_float::OrderedFloat;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VarOrConst {
    Var(VarId),
    External(String),
    // TODO: rename to Literal
    Const(OrderedFloat<f64>),
}

impl VarOrConst {
    pub fn external(&self) -> Option<&String> {
        match self {
            VarOrConst::External(s) => Some(s),
            _ => None,
        }
    }
    pub fn used_vars(&self) -> HashSet<VarId> {
        let mut ret = HashSet::default();
        if let VarOrConst::Var(id) = self {
            ret.insert(*id);
        }
        ret
    }
}

impl From<VarId> for VarOrConst {
    fn from(value: VarId) -> Self {
        VarOrConst::Var(value)
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct BlockId(pub usize);

#[derive(Default)]
pub struct Program {
    pub blocks: Vec<Block>,
}

#[derive(Default)]
pub struct Block {
    pub instructions: Vec<Instruction>,
    pub prev: Vec<BlockId>,
    pub next: Vec<BlockId>,
}

#[derive(Clone)]
pub enum Instruction {
    Assignment {
        id: VarId,
        value: VarValue,
    },
    Branch {
        // Variable that stores the 0 (false) or != 0 (true) that will be used to decide where to jump to.
        cond: VarOrConst,
        // Block where we jump to, when cond is true
        true_block: BlockId,
        // Block where we jump to, when cond is false
        false_block: BlockId,
    },
    Yield,
}

impl std::fmt::Debug for Instruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Instruction::Assignment { id, value } => {
                write!(f, "v_{} = {:?}", id.0, value)
            }
            Instruction::Branch {
                cond,
                true_block,
                false_block,
            } => {
                write!(
                    f,
                    "if {:?} {{ jump({:?} }} else {{ jump {:?} }}",
                    cond, true_block, false_block
                )
            }
            Instruction::Yield => write!(f, "yield"),
        }
    }
}

impl std::fmt::Debug for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for ins in &self.instructions {
            writeln!(f, "{:?}", ins)?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for Program {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, block) in self.blocks.iter().enumerate() {
            writeln!(f, "Block {i}")?;
            write!(f, "{:?}", block)?;
            writeln!(f, "Next: {:?}", block.next)?;
        }
        Ok(())
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct VarId(pub usize);

#[derive(Debug, Clone)]
pub enum VarValue {
    Single(VarOrConst),
    Phi(Vec<VarId>),
    BinaryOp {
        lhs: VarOrConst,
        op: BinaryOpcode,
        rhs: VarOrConst,
    },
    Call {
        name: String,
        args: Vec<VarOrConst>,
    },
}

impl VarValue {
    pub fn used_vars(&self) -> HashSet<VarId> {
        match self {
            VarValue::Single(x) => x.used_vars(),
            // TODO: not sure what to return here
            VarValue::Phi(args) => args.iter().copied().collect(),
            VarValue::BinaryOp { lhs, op: _, rhs } => {
                let mut ret = lhs.used_vars();
                ret.extend(rhs.used_vars());
                ret
            }
            VarValue::Call { name: _, args } => {
                let mut ret = HashSet::default();
                for arg in args {
                    ret.extend(arg.used_vars());
                }
                ret
            }
        }
    }
}

impl From<VarOrConst> for VarValue {
    fn from(value: VarOrConst) -> Self {
        VarValue::Single(value)
    }
}

impl From<VarId> for VarValue {
    fn from(id: VarId) -> Self {
        VarValue::Single(id.into())
    }
}
