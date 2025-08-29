use std::collections::HashMap;

use stationeers_mips::instructions::{Arithmetic, DeviceIo, Instruction, Misc};
use stationeers_mips::types::{Device, DeviceVariable, Number, Register, RegisterOrNumber};

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
    pub fn new(instructions: Vec<Instruction>) -> Self {
        Simulator {
            instructions,
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
        for i in 0..127 {
            let sp = self
                .registers
                .get(&Register::Sp)
                .copied()
                .unwrap_or(0.0)
                .round() as usize;
            let ins = match instructions.get(sp) {
                Some(x) => x,
                None => return TickResult::End,
            };
            println!("Executing `{}`", ins);
            match ins {
                Instruction::Arithmetic(x) => self.execute_arithmetic(&x),
                Instruction::DeviceIo(x) => self.execute_deviceio(&x),
                Instruction::Misc(Misc::Yield) => return TickResult::Yield,
                Instruction::Misc(x) => self.execute_misc(&x),
                _ => todo!(),
            }
            self.registers.insert(Register::Sp, (sp + 1) as f64);
        }
        return TickResult::LimitHit;
    }

    fn execute_arithmetic(&mut self, ins: &Arithmetic) {
        match &ins {
            Arithmetic::Add { register, a, b } => {
                self.registers
                    .insert(*register, self.read(a) + self.read(b));
            }
            _ => {}
        }
    }

    fn read(&self, r: &RegisterOrNumber) -> f64 {
        match r {
            RegisterOrNumber::Register(r) => self.registers.get(r).copied().unwrap_or_default(),
            RegisterOrNumber::Number(x) => x.into(),
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
                stationeers_mips::types::RegisterOrNumber::Number(Number::Int(x)) => {
                    self.registers.insert(*register, (*x).into());
                }
                _ => todo!(),
            },
            _ => todo!(),
        }
    }
}
