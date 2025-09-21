use crate::error::Error;
use crate::types::{Register, RegisterOrNumber};

/// An enum representing miscellaneous Stationeers MIPS instructions.
/// These instructions are not part of any other category.
#[derive(Clone)]
pub enum Misc {
    /// Labels register or device reference with name. device references affect what shows on the
    /// screws on the IC base
    ///
    /// alias str r?|d?
    Alias {
        /// the name of the alias
        name: String,
        /// the target the alias should point to
        target: String,
    },
    /// Creates a label that will be replaced throughout the program with the provided value
    ///
    /// define str num
    Define { name: String, value: f64 },
    /// Halt and catch fire
    ///
    /// hcf
    Halt,
    /// Register = a
    ///
    /// move r? a(r?|num)
    Move {
        register: Register,
        a: RegisterOrNumber,
    },
    /// Pause execution for a seconds
    ///
    /// sleep a(r?|num)
    Sleep { a: RegisterOrNumber },
    /// Pause execution until the next tick
    ///
    /// yield
    Yield,
    /// Marks a location in the program
    ///
    /// {name}:
    Label { name: String },
    /// Adds a comment to the program
    ///
    /// # {comment}
    Comment { comment: String },
}

impl std::fmt::Display for Misc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Misc::Alias { name, target } => write!(f, "alias {name} {target}"),
            Misc::Define { name, value } => write!(f, "define {name} {value}"),
            Misc::Halt => write!(f, "hcf"),
            Misc::Move { register, a } => write!(f, "move {register} {a}"),
            Misc::Sleep { a } => write!(f, "sleep {a}"),
            Misc::Yield => write!(f, "yield"),
            Misc::Label { name } => write!(f, "{name}:"),
            Misc::Comment { comment } => write!(f, "# {comment}"),
        }
    }
}

impl std::str::FromStr for Misc {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split_whitespace();

        let command = parts
            .next()
            .ok_or_else(|| Error::ParseError(s.to_string()))?;

        match command {
            "yield" => Ok(Misc::Yield),
            "move" => {
                let register = parts
                    .next()
                    .ok_or_else(|| Error::ParseError(s.to_string()))?
                    .parse()?;
                let value = parts
                    .next()
                    .ok_or_else(|| Error::ParseError(s.to_string()))?
                    .parse()?;
                Ok(Misc::Move { register, a: value })
            }
            _ => Err(Error::ParseError(s.to_string())),
        }
    }
}
