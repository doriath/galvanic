use std::collections::HashMap;

use stationeers_mips::instructions::{DeviceIo, Instruction, Misc};
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
                Instruction::DeviceIo(x) => self.execute_deviceio(&x),
                Instruction::Misc(Misc::Yield) => return TickResult::Yield,
                Instruction::Misc(x) => self.execute_misc(&x),
                _ => todo!(),
            }
            self.registers.insert(Register::Sp, (sp + 1) as f64);
        }
        return TickResult::LimitHit;
    }

    fn execute_deviceio(&mut self, ins: &DeviceIo) {
        match &ins {
            DeviceIo::StoreDeviceVariable {
                device,
                variable,
                register,
            } => {
                let value: f64 = match register {
                    RegisterOrNumber::Register(r) => {
                        self.registers.get(r).copied().unwrap_or_default()
                    }
                    RegisterOrNumber::Number(x) => x.into(),
                };

                println!("Value = {}", value);
                self.devices
                    .entry(device.clone())
                    .or_default()
                    .insert(variable.clone(), value);
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
