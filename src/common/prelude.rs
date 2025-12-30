pub use super::{
    bus_device::{BusDevice as _, BusDeviceWithAddress as _},
    dma::DmaChannel as _,
    fugit::{ExtU32 as _, RateExtU32 as _},
    i2c::I2cPeriph as _,
    timer::{
        GeneralTimer as _, PwmChannel as _, TimerDirection as _, TimerWithPwm as _,
        TimerWithPwm1Ch as _, TimerWithPwm2Ch as _, TimerWithPwm3Ch as _, TimerWithPwm4Ch as _,
    },
    uart::{UartPeriph as _, UartPeriphWithDma as _},
};
pub use os_trait::prelude::*;
