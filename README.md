# stm32f1-hal

[![CI](https://github.com/mcu-rust/stm32f1-hal/workflows/CI/badge.svg)](https://github.com/mcu-rust/stm32f1-hal/actions)
[![Crates.io](https://img.shields.io/crates/v/stm32f1-hal.svg)](https://crates.io/crates/stm32f1-hal)
[![Docs.rs](https://docs.rs/stm32f1-hal/badge.svg)](https://docs.rs/stm32f1-hal)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](./LICENSE)
[![Downloads](https://img.shields.io/crates/d/stm32f1-hal.svg)](https://crates.io/crates/stm32f1-hal)

**stm32f1-hal** is a Rust Hardware Abstraction Layer (HAL) for **STM32F1 microcontrollers** (All F1 series devices). It implements selected [embedded-hal](https://github.com/rust-embedded/embedded-hal) traits to provide a clear, idiomatic interface for embedded development on STM32F1. Many parts are adapted from [stm32f1xx-hal](https://github.com/stm32-rs/stm32f1xx-hal), with a focus on readability and maintainability.

## 🎯 Motivation
Existing crates didn’t fully meet my needs:
- **[stm32f1xx-hal](https://github.com/stm32-rs/stm32f1xx-hal)**’s design didn’t align with my workflow.
- **[stm32-hal](https://github.com/David-OConnor/stm32-hal)** lacks support for the STM32F1 series.

To address this gap, I created **[stm32f1-hal](https://github.com/mcu-rust/stm32f1-hal)**.
While parts of the implementation are adapted from [stm32f1xx-hal](https://github.com/stm32-rs/stm32f1xx-hal), the focus here is on clarity, readability, and usability.

## 📖 Design Philosophy
- **Readability is the most important.**
  We only write code a few times, but we read it countless times. Clear understanding is essential for long-term maintenance.

- **Concise is not equal to simple.**
  Fewer lines of code do not necessarily mean easier to read or understand.

- **Prefer [sync-code](https://crates.io/crates/sync-code) over complex macros + generics.**
  In complex modules, combining macros with generics often makes the code harder to follow and maintain.
  Instead, I use [sync-code](https://crates.io/crates/sync-code), a preprocessing tool that replaces annotated comments with reusable code blocks, keeping peripheral code consistent and readable.

- In addition, a script is used to generate code for GPIO alternate function remapping.

## 📦 Usage
```shell
cargo add stm32f1-hal
```

```rust
use stm32f1_hal as hal;
use hal::pac;
use hal::prelude::*;

fn main() {
    let dp = pac::Peripherals::take().unwrap();
    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.constrain();

    let clocks = rcc.cfgr.freeze(&mut flash.acr);
    let mut gpioa = dp.GPIOA.split(&mut rcc.apb2);

    let mut led = gpioa.pa5.into_push_pull_output(&mut gpioa.crl);

    loop {
        led.set_high();
        // delay...
        led.set_low();
        // delay...
    }
}
```
See [example](examples/f103c8/src/main.rs).
See [crate](https://crates.io/crates/stm32f1-hal).

## 🗺 Roadmap
**This project is still in its early stages, with only a few features implemented so far**. Contributions and feedback are welcome to help expand support for more peripherals and features.
- [x] GPIO
- [x] UART
- [ ] I2C
- [ ] ADC
- [ ] DMA
- [ ] More features

## 🛠 Contributing
- Open issues for bugs or feature requests.
- Submit PRs with improvements or new peripheral support (I2C, ADC, DMA, etc.).

## 🔖 Keywords
**stm32 · stm32f1 · rust · embedded-hal · hal · microcontroller · embedded development**
