pub use super::{
    dma::DmaChannel,
    i2c::I2cPeriph,
    os::*,
    timer::{
        GeneralTimer, PwmChannel, TimerDirection, TimerWithPwm, TimerWithPwm1Ch, TimerWithPwm2Ch,
        TimerWithPwm3Ch, TimerWithPwm4Ch,
    },
    uart::{UartPeriph, UartPeriphWithDma},
};
