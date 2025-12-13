pub use super::{
    bus_device::{BusDevice, BusDeviceWithAddress},
    dma::DmaChannel,
    i2c::{I2cBusDevice, I2cPeriph},
    timer::{
        GeneralTimer, PwmChannel, TimerDirection, TimerWithPwm, TimerWithPwm1Ch, TimerWithPwm2Ch,
        TimerWithPwm3Ch, TimerWithPwm4Ch,
    },
    uart::{UartPeriph, UartPeriphWithDma},
};
pub use os_trait::prelude::*;
