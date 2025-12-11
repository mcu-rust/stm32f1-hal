use crate::pac::Interrupt;
use cortex_m::peripheral::NVIC;
use cortex_m::peripheral::SCB;

pub trait ScbInit {
    fn init(self) -> Scb;
}
pub struct Scb {
    pub(crate) scb: SCB,
}
impl ScbInit for SCB {
    fn init(self) -> Scb {
        Scb { scb: self }
    }
}

pub trait NvicInit {
    fn init(self) -> Nvic;
}
pub struct Nvic {
    pub(crate) nvic: NVIC,
}
impl NvicInit for NVIC {
    fn init(self) -> Nvic {
        Nvic { nvic: self }
    }
}

const SCB_AIRCR_VECTKEY_MASK: u32 = 0xFFFF << 16;
const SCB_AIRCR_VECTKEY: u32 = 0x05FA << 16;
const SCB_AIRCR_PRIGROUP_MASK: u32 = 0x7 << 8;
// const SCB_AIRCR_SYSRESETREQ: u32 = 1 << 2;

impl Scb {
    /// Call it as early as possible.
    /// It's best to use Group4.
    pub fn set_priority_grouping(&mut self, grouping: PriorityGrouping) {
        let mask = !(SCB_AIRCR_VECTKEY_MASK | SCB_AIRCR_PRIGROUP_MASK);
        let grouping: u32 = grouping.into();
        cortex_m::asm::dsb();
        unsafe {
            self.scb
                .aircr
                .modify(|r| (r & mask) | SCB_AIRCR_VECTKEY | grouping)
        };
        cortex_m::asm::dsb();
    }

    pub fn get_priority_grouping(&self) -> PriorityGrouping {
        self.scb.aircr.read().into()
    }
}

impl Nvic {
    /// p = 0 ~ 15, The smaller the number, the higher the priority.
    /// It combines preemption and sub priority based on the grouping.
    pub fn set_priority(&mut self, it: Interrupt, priority: u8) {
        unsafe {
            // only use the highest 4 bits
            self.nvic.set_priority(it, priority << 4);
        }
    }

    /// Enable or disable a interrupt.
    pub fn enable(&mut self, it: Interrupt, en: bool) {
        if en {
            unsafe {
                NVIC::unmask(it);
            }
        } else {
            NVIC::mask(it);
        }
    }
}

pub enum PriorityGrouping {
    /// 0 bits for preemption priority
    /// 4 bits for sub priority
    Group0,
    /// 1 bits for preemption priority
    /// 3 bits for sub priority
    Group1,
    /// 2 bits for preemption priority
    /// 2 bits for sub priority
    Group2,
    /// 3 bits for preemption priority
    /// 1 bits for sub priority
    Group3,
    /// Default used: 4 bits for preemption priority
    /// 0 bits for sub priority
    Group4,
    Unknown(u8),
}

impl From<PriorityGrouping> for u32 {
    fn from(value: PriorityGrouping) -> Self {
        match value {
            PriorityGrouping::Group0 => 7 << 8,
            PriorityGrouping::Group1 => 6 << 8,
            PriorityGrouping::Group2 => 5 << 8,
            PriorityGrouping::Group3 => 4 << 8,
            PriorityGrouping::Group4 => 3 << 8,
            PriorityGrouping::Unknown(v) => ((v as u32) << 8) & SCB_AIRCR_PRIGROUP_MASK,
        }
    }
}

impl From<u32> for PriorityGrouping {
    fn from(value: u32) -> Self {
        match (value & SCB_AIRCR_PRIGROUP_MASK) >> 8 {
            7 => Self::Group0,
            6 => Self::Group1,
            5 => Self::Group2,
            4 => Self::Group3,
            3 => Self::Group4,
            v => PriorityGrouping::Unknown(v as u8),
        }
    }
}
