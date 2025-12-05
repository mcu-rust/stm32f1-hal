# stm32f1-hal
This crate is the Hardware Abstraction Layer (HAL) for the STM32 F1 series of MCUs.

Because the design of [stm32f1xx-hal](https://github.com/stm32-rs/stm32f1xx-hal) is unsuitable for my needs and [stm32-hal](https://github.com/David-OConnor/stm32-hal) doesn't support the F1 series, I decided to write a new crate.
Many codes come from [stm32f1xx-hal](https://github.com/stm32-rs/stm32f1xx-hal).

## Design
Below are the design principles.
1. Readability is the most important.
    - We only write code a few times, but we read it countless times. Moreover, understanding the code is a necessary condition for maintaining it.
2. Concise is not equal to simple.
    - Fewer lines of code do not necessarily mean easier to read and understand.

Therefore, if a module is quite complex, I would not use a `macro` + `generic` approach, as it is too difficult to read.

Instead, I use [sync-code](https://crates.io/crates/sync-code) to manage duplicate code across peripherals and a script to generate code for GPIO alternate function remapping.

## Usage
```shell
cargo add stm32f1-hal
```

See [example](examples/f103c8/src/main.rs).

See [crate](https://crates.io/crates/stm32f1-hal).

## Note
This project is still in its early stages, with only a few features completed.
