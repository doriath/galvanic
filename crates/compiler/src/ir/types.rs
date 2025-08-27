use ayysee_parser::ast::BinaryOpcode;

#[derive(Debug, Clone)]
pub enum VarOrConst {
    Var(VarId),
    External(String),
    Const(f64),
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

pub enum Instruction {
    Assignment {
        id: VarId,
        value: VarValue,
    },
    Branch {
        // Variable that stores the 0 (false) or != 0 (true) that will be used to decide where to jump to.
        cond: VarId,
        // Block where we jump to, when cond is true
        true_block: BlockId,
        // Block where we jump to, when cond is false
        false_block: BlockId,
    },
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
        }
        Ok(())
    }
}

#[derive(Debug, Copy, Clone)]
pub struct VarId(pub usize);

#[derive(Debug, Clone)]
pub enum VarValue {
    Single(VarOrConst),
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
