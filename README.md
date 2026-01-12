# stm32f1-hal

[![CI](https://github.com/mcu-rust/stm32f1-hal/workflows/CI/badge.svg)](https://github.com/mcu-rust/stm32f1-hal/actions)
[![Crates.io](https://img.shields.io/crates/v/stm32f1-hal.svg)](https://crates.io/crates/stm32f1-hal)
[![Docs.rs](https://docs.rs/stm32f1-hal/badge.svg)](https://docs.rs/stm32f1-hal)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](./LICENSE)
[![Downloads](https://img.shields.io/crates/d/stm32f1-hal.svg)](https://crates.io/crates/stm32f1-hal)

**stm32f1-hal** is a Rust Hardware Abstraction Layer (HAL) for **STM32F1 microcontrollers** (All F1 series devices). It provides a clear, idiomatic interface for embedded development on STM32F1.

- It implements selected [embedded-hal](https://github.com/rust-embedded/embedded-hal) traits.
- It uses the [os-trait](https://crates.io/crates/os-trait) crate, which makes it easy to integrate with different RTOSes.
- It works with stable Rust.

## 🎯 Motivation
Existing crates didn’t fully meet my needs:
- [stm32f1xx-hal](https://github.com/stm32-rs/stm32f1xx-hal)’s design didn’t align with my workflow.
- [stm32-hal](https://github.com/David-OConnor/stm32-hal) lacks support for the STM32F1 series.
- [Embassy](https://github.com/embassy-rs/embassy) and [RTIC](https://github.com/rtic-rs/rtic) are async frameworks, but I need a sync one.

To address this gap, I created **[stm32f1-hal](https://github.com/mcu-rust/stm32f1-hal)**.
While parts of the implementation are adapted from [stm32f1xx-hal](https://github.com/stm32-rs/stm32f1xx-hal), the focus here is on clarity, readability, and usability.

## 📖 Design Philosophy
- **Readability is the most important.**
  We only write code a few times, but we read it countless times. Clear understanding is essential for long-term maintenance.
  - **Prefer [sync-code](https://crates.io/crates/sync-code) over complex macros**
    In complex modules, combining macros with generics and calling a lot of low level interfaces often makes the code harder to follow and maintain.
    Instead, I use [sync-code](https://crates.io/crates/sync-code) to synchronizes code blocks across peripherals, keeping peripheral code easy to read and maintain.

  - A script is used to generate code for GPIO alternate function remapping.

- **Concise is not equal to simple.**
  Fewer lines of code do not necessarily mean easier to read or understand.
  - The initialization code is not hidden. This makes the `main` function more verbose, but everything that’s happening is clearly visible.
  - Static variables are kept to a minimum in the library.

## 📦 Usage
```shell
cargo add stm32f1-hal
```

```rust
use stm32f1_hal::{self as hal, pac, cortex_m_rt::entry, prelude::*};

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();
    let mut flash = dp.FLASH.init();

    let cfg = rcc::Config::default();
    let mut rcc = dp.RCC.init().freeze(cfg, &mut flash.acr);
    let mut gpioa = dp.GPIOA.split(&mut rcc);

    let mut led = gpioa.pa5.into_push_pull_output(&mut gpioa.crl);

    loop {
        led.set_high();
        // delay...
        led.set_low();
        // delay...
    }
}
```

### Examples

For a more complete example, see [example](examples/f103c8/src/main.rs).
And [stm32f1-FreeRTOS-example](https://github.com/mcu-rust/stm32f1-FreeRTOS-example) shows how to use this crate with FreeRTOS together.

## 🗺 Roadmap
**This project is still in its early stages, with only a few features implemented so far**. Contributions and feedback are welcome to help expand support for more peripherals and features.
- [x] GPIO (tested)
- [x] EXTI (tested)
- [x] UART + poll mode (tested)
- [x] UART + interrupt (stress tested)
- [x] UART + DMA (stress tested)
- [x] I2C + interrupt (tested)
- [x] SPI + interrupt (tested)
- [x] DMA
- [ ] ADC
- [ ] More features

## 🛠 Contributing
- Submit PRs with documents, improvements or new peripheral support.
- Open issues for bugs or feature requests.

## 🔖 Keywords
**stm32 · stm32f1 · rust · embedded-hal · hal · microcontroller · embedded development**
