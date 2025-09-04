use std::collections::HashMap;

use stationeers_mips::instructions::{
    Arithmetic, DeviceIo, FlowControl, Instruction, Misc, VariableSelection,
};
use stationeers_mips::types::{Device, DeviceVariable, Register, RegisterOrNumber};
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
                stationeers_mips::types::RegisterOrNumber::Number(x) => {
                    let f: f64 = x.into();
                    self.registers.insert(*register, f);
                }
                _ => todo!(),
            },
            _ => todo!(),
        }
    }
    fn execute_select(&mut self, ins: &VariableSelection) {
        match ins {
            VariableSelection::SelectApproximatelyEqual { register, a, b, c } => todo!(),
            VariableSelection::SelectApproximatelyZero { register, a, b } => todo!(),
            VariableSelection::SelectDeviceNotSet { register, d } => todo!(),
            VariableSelection::SelectDeviceSet { register, d } => todo!(),
            VariableSelection::Select { register, a, b, c } => todo!(),
            VariableSelection::SelectEqual { register, a, b } => todo!(),
            VariableSelection::SelectEqualZero { register, a } => todo!(),
            VariableSelection::SelectGreaterOrEqual { register, a, b } => todo!(),
            VariableSelection::SelectGreaterOrEqualZero { register, a } => todo!(),
            VariableSelection::SelectGreaterThan { register, a, b } => {
                self.registers
                    .insert(*register, (self.read(a) > self.read(b)) as i32 as f64);
            }
            VariableSelection::SelectGreaterThanZero { register, a } => todo!(),
            VariableSelection::SelectLessOrEqual { register, a, b } => todo!(),
            VariableSelection::SelectLessOrEqualZero { register, a } => todo!(),
            VariableSelection::SelectLessThan { register, a, b } => todo!(),
            VariableSelection::SelectLessThanZero { register, a } => todo!(),
            VariableSelection::SelectNotApproximatelyEqual { register, a, b, c } => todo!(),
            VariableSelection::SelectNotApproximatelyZero { register, a, b } => todo!(),
            VariableSelection::SelectNotEqual { register, a, b } => todo!(),
            VariableSelection::SelectNotEqualZero { register, a } => todo!(),
        }
    }
    fn execute_flow(&mut self, ins: &FlowControl) {
        match ins {
            FlowControl::BranchAbsoluteLessThan { a, b, c, d } => todo!(),
            FlowControl::BranchAbsoluteLessThanAndLink { a, b, c, d } => todo!(),
            FlowControl::BranchAbsoluteZero { a, b, c } => todo!(),
            FlowControl::BranchAbsoluteZeroAndLink { a, b, c } => todo!(),
            FlowControl::BranchEqual { a, b, c } => todo!(),
            FlowControl::BranchEqualAndLink { a, b, c } => todo!(),
            FlowControl::BranchEqualZero { a, b } => todo!(),
            FlowControl::BranchEqualZeroAndLink { a, b } => todo!(),
            FlowControl::BranchGreaterOrEqual { a, b, c } => todo!(),
            FlowControl::BranchGreaterOrEqualAndLink { a, b, c } => todo!(),
            FlowControl::BranchGreaterOrEqualZero { a, b } => todo!(),
            FlowControl::BranchGreaterOrEqualZeroAndLink { a, b } => todo!(),
            FlowControl::BranchGreaterThan { a, b, c } => todo!(),
            FlowControl::BranchGreaterThanAndLink { a, b, c } => todo!(),
            FlowControl::BranchGreaterThanZero { a, b } => todo!(),
            FlowControl::BranchGreaterThanZeroAndLink { a, b } => todo!(),
            FlowControl::BranchLessOrEqual { a, b, c } => todo!(),
            FlowControl::BranchLessOrEqualAndLink { a, b, c } => todo!(),
            FlowControl::BranchLessOrEqualZero { a, b } => todo!(),
            FlowControl::BranchLessOrEqualZeroAndLink { a, b } => todo!(),
            FlowControl::BranchLessThan { a, b, c } => todo!(),
            FlowControl::BranchLessThanAndLink { a, b, c } => todo!(),
            FlowControl::BranchLessThanZero { a, b } => todo!(),
            FlowControl::BranchLessThanZeroAndLink { a, b } => todo!(),
            FlowControl::BranchNotApproximatelyEqual { a, b, c, d } => todo!(),
            FlowControl::BranchNotApproximatelyEqualAndLink { a, b, c, d } => todo!(),
            FlowControl::BranchNotApproximatelyZero { a, b, c } => todo!(),
            FlowControl::BranchNotApproximatelyZeroAndLink { a, b, c } => todo!(),
            FlowControl::BranchNotEqual { a, b, c } => todo!(),
            FlowControl::BranchNotEqualAndLink { a, b, c } => todo!(),
            FlowControl::BranchNotEqualZero { a, b } => todo!(),
            FlowControl::BranchNotEqualZeroAndLink { a, b } => todo!(),
            FlowControl::RelativeBranchApproximatelyEqual { a, b, c, d } => todo!(),
            FlowControl::RelativeBranchApproximatelyZero { a, b, c } => todo!(),
            FlowControl::RelativeBranchEqual { a, b, c } => todo!(),
            FlowControl::RelativeBranchEqualZero { a, b } => {
                if self.read(a) == 0.0 {
                    println!("if false");
                    let idx = self.read(b) as i32;
                    self.registers.insert(Register::Sp, (idx - 1) as f64);
                } else {
                    println!("if true");
                }
            }
            FlowControl::RelativeBranchGreaterOrEqual { a, b, c } => todo!(),
            FlowControl::RelativeBranchGreaterOrEqualZero { a, b } => todo!(),
            FlowControl::RelativeBranchGreaterThan { a, b, c } => todo!(),
            FlowControl::RelativeBranchGreaterThanZero { a, b } => todo!(),
            FlowControl::RelativeBranchLessOrEqual { a, b, c } => todo!(),
            FlowControl::RelativeBranchLessOrEqualZero { a, b } => todo!(),
            FlowControl::RelativeBranchLessThan { a, b, c } => todo!(),
            FlowControl::RelativeBranchLessThanZero { a, b } => todo!(),
            FlowControl::RelativeBranchNotApproximatelyEqual { a, b, c, d } => todo!(),
            FlowControl::RelativeBranchNotApproximatelyZero { a, b, c } => todo!(),
            FlowControl::RelativeBranchNotEqual { a, b, c } => todo!(),
            FlowControl::RelativeBranchNotEqualZero { a, b } => todo!(),
            FlowControl::Jump { a } => {
                self.registers.insert(Register::Sp, (a - 1) as f64);
            }
            FlowControl::JumpAndLink { a } => todo!(),
            FlowControl::JumpRelative { a } => todo!(),
        }
    }
}
