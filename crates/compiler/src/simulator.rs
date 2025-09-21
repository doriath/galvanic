use std::collections::HashMap;

use stationeers_mips::instructions::{
    Arithmetic, DeviceIo, FlowControl, Instruction, Logic, Misc, VariableSelection,
};
use stationeers_mips::types::{Device, DeviceVariable, JumpDest, Register, RegisterOrNumber};
use stationeers_mips::Program;

pub struct Simulator {
    instructions: Vec<Instruction>,
    state: State,
}

struct State {
    registers: HashMap<Register, f64>,
    devices: HashMap<Device, HashMap<DeviceVariable, f64>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TickResult {
    Yield,
    LimitHit,
    End,
}

impl Simulator {
    pub fn new(program: Program) -> Self {
        Simulator {
            instructions: program.instructions,
            state: State {
                registers: HashMap::default(),
                devices: HashMap::default(),
            },
        }
    }

    pub fn tick(&mut self) -> TickResult {
        self.state.tick(&self.instructions)
    }

    pub fn read(&self, d: Device, logic_type: DeviceVariable) -> f64 {
        if let Some(x) = self.state.devices.get(&d) {
            return x.get(&logic_type).copied().unwrap_or(0.0);
        }
        0.0
    }
    pub fn write(&mut self, d: Device, logic_type: DeviceVariable, v: f64) {
        self.state
            .devices
            .entry(d)
            .or_default()
            .insert(logic_type, v);
    }
}

impl State {
    fn tick(&mut self, instructions: &[Instruction]) -> TickResult {
        for _ in 0..127 {
            let ins = match instructions.get(self.sp() as usize) {
                Some(x) => x,
                None => return TickResult::End,
            };
            println!("Executing `{}`", ins);
            match ins {
                Instruction::Arithmetic(x) => self.execute_arithmetic(&x),
                Instruction::DeviceIo(x) => self.execute_deviceio(&x),
                Instruction::Misc(Misc::Yield) => {
                    self.set_sp(self.sp() + 1);
                    return TickResult::Yield;
                }
                Instruction::Misc(x) => self.execute_misc(&x),
                Instruction::VariableSelection(x) => self.execute_select(&x),
                Instruction::FlowControl(x) => self.execute_flow(&x),
                Instruction::Logic(x) => self.execute_logic(&x),
                _ => todo!("{}", ins),
            }
            self.set_sp(self.sp() + 1);
        }
        return TickResult::LimitHit;
    }

    fn sp(&self) -> i32 {
        self.registers
            .get(&Register::Sp)
            .copied()
            .unwrap_or(0.0)
            .round() as i32
    }
    fn set_sp(&mut self, sp: i32) {
        self.registers.insert(Register::Sp, sp as f64);
    }

    fn read(&self, r: &RegisterOrNumber) -> f64 {
        match r {
            RegisterOrNumber::Register(r) => self.registers.get(r).copied().unwrap_or_default(),
            RegisterOrNumber::Number(x) => *x,
        }
    }

    fn read_bool(&self, v: &RegisterOrNumber) -> bool {
        self.read(v) != 0.0
    }

    fn execute_logic(&mut self, ins: &Logic) {
        match &ins {
            Logic::And { register, a, b } => {
                self.registers
                    .insert(*register, (self.read_bool(a) && self.read_bool(b)).into());
            }
            Logic::Or { register, a, b } => {
                self.registers
                    .insert(*register, (self.read_bool(a) || self.read_bool(b)).into());
            }
            _ => todo!(),
        }
    }

    fn execute_arithmetic(&mut self, ins: &Arithmetic) {
        match &ins {
            Arithmetic::Add { register, a, b } => {
                self.registers
                    .insert(*register, self.read(a) + self.read(b));
            }
            Arithmetic::Divide { register, a, b } => {
                self.registers
                    .insert(*register, self.read(a) / self.read(b));
            }
            Arithmetic::Multiply { register, a, b } => {
                self.registers
                    .insert(*register, self.read(a) * self.read(b));
            }
            Arithmetic::Subtract { register, a, b } => {
                self.registers
                    .insert(*register, self.read(a) - self.read(b));
            }
            _ => todo!(),
        }
    }
    fn execute_deviceio(&mut self, ins: &DeviceIo) {
        match &ins {
            DeviceIo::StoreDeviceVariable {
                device,
                variable,
                register,
            } => {
                let value: f64 = self.read(register);
                self.devices
                    .entry(device.clone())
                    .or_default()
                    .insert(variable.clone(), value);
            }
            DeviceIo::LoadDeviceVariable {
                register,
                device,
                variable,
            } => {
                let value = self
                    .devices
                    .entry(device.clone())
                    .or_default()
                    .get(&variable)
                    .copied()
                    .unwrap_or_default();
                self.registers.insert(register.clone(), value);
            }
            _ => todo!(),
        }
    }
    fn execute_misc(&mut self, ins: &Misc) {
        match &ins {
            Misc::Move { register, a } => match a {
                stationeers_mips::types::RegisterOrNumber::Number(x) => {
                    self.registers.insert(*register, *x);
                }
                _ => todo!(),
            },
            _ => todo!(),
        }
    }
    fn execute_select(&mut self, ins: &VariableSelection) {
        match ins {
            VariableSelection::SelectApproximatelyEqual { register, a, b, c } => {
                self.registers.insert(
                    *register,
                    if (self.read(a) - self.read(b)).abs() < self.read(c) {
                        1.0
                    } else {
                        0.0
                    },
                );
            }
            VariableSelection::SelectApproximatelyZero { register, a, b } => {
                self.registers.insert(
                    *register,
                    if self.read(a).abs() < self.read(b) {
                        1.0
                    } else {
                        0.0
                    },
                );
            }
            VariableSelection::Select { register, a, b, c } => {
                self.registers.insert(
                    *register,
                    if self.read(a) != 0.0 {
                        self.read(b)
                    } else {
                        self.read(c)
                    },
                );
            }
            VariableSelection::SelectEqual { register, a, b } => {
                self.registers
                    .insert(*register, (self.read(a) == self.read(b)) as i32 as f64);
            }
            VariableSelection::SelectEqualZero { register, a } => {
                self.registers
                    .insert(*register, (self.read(a) == 0.0) as i32 as f64);
            }
            VariableSelection::SelectGreaterOrEqual { register, a, b } => {
                self.registers
                    .insert(*register, (self.read(a) >= self.read(b)) as i32 as f64);
            }
            VariableSelection::SelectGreaterOrEqualZero { register, a } => {
                self.registers
                    .insert(*register, (self.read(a) >= 0.0) as i32 as f64);
            }
            VariableSelection::SelectGreaterThan { register, a, b } => {
                self.registers
                    .insert(*register, (self.read(a) > self.read(b)) as i32 as f64);
            }
            VariableSelection::SelectGreaterThanZero { register, a } => {
                self.registers
                    .insert(*register, (self.read(a) > 0.0) as i32 as f64);
            }
            VariableSelection::SelectLessOrEqual { register, a, b } => {
                self.registers
                    .insert(*register, (self.read(a) <= self.read(b)) as i32 as f64);
            }
            VariableSelection::SelectLessOrEqualZero { register, a } => {
                self.registers
                    .insert(*register, (self.read(a) <= 0.0) as i32 as f64);
            }
            VariableSelection::SelectLessThan { register, a, b } => {
                self.registers
                    .insert(*register, (self.read(a) < self.read(b)) as i32 as f64);
            }
            VariableSelection::SelectLessThanZero { register, a } => {
                self.registers
                    .insert(*register, (self.read(a) < 0.0) as i32 as f64);
            }
            VariableSelection::SelectNotApproximatelyEqual { register, a, b, c } => {
                self.registers.insert(
                    *register,
                    if (self.read(a) - self.read(b)).abs() >= self.read(c) {
                        1.0
                    } else {
                        0.0
                    },
                );
            }
            VariableSelection::SelectNotApproximatelyZero { register, a, b } => {
                self.registers.insert(
                    *register,
                    if self.read(a).abs() >= self.read(b) {
                        1.0
                    } else {
                        0.0
                    },
                );
            }
            VariableSelection::SelectNotEqual { register, a, b } => {
                self.registers
                    .insert(*register, (self.read(a) != self.read(b)) as i32 as f64);
            }
            VariableSelection::SelectNotEqualZero { register, a } => {
                self.registers
                    .insert(*register, (self.read(a) != 0.0) as i32 as f64);
            }
            _ => todo!(),
        }
    }
    fn execute_flow(&mut self, ins: &FlowControl) {
        match ins {
            FlowControl::BranchEqualZero { a, b } => {
                if self.read(a) == 0.0 {
                    let idx = self.read(b) as i32;
                    self.registers.insert(Register::Sp, (idx - 1) as f64);
                }
            }
            FlowControl::Jump { a } => {
                match a {
                    JumpDest::Label(_) => unimplemented!(),
                    JumpDest::Register(r) => {
                        self.registers
                            .insert(Register::Sp, self.read(&(r.clone().into())) - 1.0);
                    }
                    JumpDest::Number(a) => {
                        self.registers.insert(Register::Sp, a - 1.0);
                    }
                };
            }
            _ => todo!(),
        }
    }
}
