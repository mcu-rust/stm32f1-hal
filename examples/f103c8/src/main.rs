#![no_std]
#![no_main]
#![allow(unused)]

mod i2c_task;
mod led_task;
mod os;
mod spi_task;
mod uart_task;

use core::panic::PanicInfo;
use cortex_m_rt::{ExceptionFrame, entry, exception};
use defmt_rtt as _;
use i2c_task::I2cTask;
use led_task::LedTask;
use os::*;
use spi_task::SpiTask;
use uart_task::UartLoopBackTask;

// Basic
use hal::{Mcu, cortex_m::asm, gpio::PinState, pac, rcc};

use hal::{
    afio::{NONE_PIN, RemapDefault},
    common::simplest_heap::Heap,
    dma::DmaPriority,
    embedded_hal::{self, pwm::SetDutyCycle},
    embedded_io,
    fugit::ExtU32,
    gpio::{Edge, ExtiPin},
    i2c::I2cMutexDevice,
    nvic_scb::PriorityGrouping,
    pac::Interrupt,
    spi,
    time::MonoTimer,
    timer::{CountDirection, PwmMode, PwmPolarity},
    uart,
};

#[global_allocator]
static HEAP: Heap<10_000> = Heap::new();

#[entry]
fn main() -> ! {
    // Clock --------------------------------------------------------

    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();
    let mut flash = dp.FLASH.init();
    let sysclk = 72.MHz();
    let cfg = rcc::Config::hse(8.MHz()).sysclk(sysclk);
    let mut rcc = dp.RCC.init().freeze(cfg, &mut flash.acr);
    assert_eq!(rcc.clocks().sysclk(), sysclk);

    // Prepare ------------------------------------------------------

    let afio = dp.AFIO.init(&mut rcc);
    let mut mcu = Mcu::new(rcc, afio, cp.SCB.init(), cp.NVIC.init(), dp.EXTI);

    // Keep them in one place for easier management
    mcu.scb.set_priority_grouping(PriorityGrouping::Group4);
    mcu.nvic.set_priority(Interrupt::I2C1_EV, 1, true);
    mcu.nvic.set_priority(Interrupt::I2C1_ER, 1, true);
    mcu.nvic.set_priority(Interrupt::EXTI1, 2, true);
    mcu.nvic.set_priority(Interrupt::USART1, 3, true);
    mcu.nvic.set_priority(Interrupt::DMA1_CHANNEL4, 3, true);
    mcu.nvic.set_priority(Interrupt::DMA1_CHANNEL5, 3, true);
    mcu.nvic.set_priority(Interrupt::SPI1, 3, true);

    // Peripherals --------------------------------------------------

    let mut sys_timer = cp.SYST.counter_hz(&mcu);
    sys_timer.start(20.Hz()).unwrap();
    // let mono_timer = MonoTimer::new(cp.DWT, cp.DCB, &mcu.rcc);

    let mut gpioa = dp.GPIOA.split(&mut mcu.rcc);
    let mut gpiob = dp.GPIOB.split(&mut mcu.rcc);
    let mut dma1 = dp.DMA1.split(&mut mcu.rcc);

    // LED ----------------------------------------------------------

    let led = gpiob.pb0.into_open_drain_output_with_state(PinState::High);
    let mut led_task = LedTask::new(led);

    // UART ---------------------------------------------------------

    #[cfg(feature = "uart")]
    let (Some(uart_tx), Some(uart_rx)) = ({
        let pin_tx = gpioa.pa9;
        let pin_rx = gpioa.pa10;
        // let pin_tx = gpiob.pb6;
        // let pin_rx = gpiob.pb7;
        // let pin_rx = hal::afio::NONE_PIN;

        let config = uart::Config::default();
        dp.USART1
            .init::<OS>(&mut mcu)
            .into_tx_rx((pin_tx, pin_rx), config, &mut mcu)
    }) else {
        panic!()
    };

    #[cfg(feature = "uart-dma")]
    let (tx, rx) = {
        dma1.4.set_priority(DmaPriority::Medium);
        dma1.5.set_priority(DmaPriority::Medium);
        let (tx, mut tx_it) = uart_tx.into_dma_ringbuf(dma1.4, 32, 0.micros());
        let (rx, mut rx_it, mut idle_it) = uart_rx.into_dma_circle(dma1.5, 64, 100.micros());
        its::DMA1_CH4_CB.set(&mut mcu, move || tx_it.interrupt_reload());
        its::DMA1_CH5_CB.set(&mut mcu, move || rx_it.interrupt_notify());
        its::USART1_CB.set(&mut mcu, move || idle_it.interrupt_notify());
        (tx, rx)
    };

    #[cfg(feature = "uart-it")]
    let (tx, rx) = {
        let (tx, mut tx_it) = uart_tx.into_interrupt(32, 0.micros());
        let (rx, mut rx_it) = uart_rx.into_interrupt(64, 100.micros());
        its::USART1_CB.set(&mut mcu, move || {
            rx_it.handler();
            tx_it.handler();
        });
        (tx, rx)
    };

    #[cfg(feature = "uart-poll")]
    let (_, _) = (uart_tx.into_poll(0.micros()), uart_rx.into_poll(0.micros()));

    #[cfg(feature = "uart")]
    let mut uart_task = UartLoopBackTask::new(tx, rx);

    // I2C ----------------------------------------------------------

    #[cfg(feature = "i2c")]
    let pins = (gpiob.pb6, gpiob.pb7);
    #[cfg(feature = "i2c-it-sole")]
    let (dev, mut it, mut it_err) =
        dp.I2C1
            .init::<OS>(&mut mcu)
            .into_interrupt_sole(pins, 200.kHz(), 4, &mut mcu);

    #[cfg(feature = "i2c-it-bus")]
    let (dev, mut it, mut it_err) = {
        let (bus, it, err_it) =
            dp.I2C1
                .init::<OS>(&mut mcu)
                .into_interrupt_bus(pins, 200.kHz(), 4, &mut mcu);
        (bus.new_device(), it, err_it)
    };
    #[cfg(feature = "i2c")]
    let mut i2c_task = {
        its::I2C1_EVENT_CB.set(&mut mcu, move || it.handler());
        its::I2C1_ERR_CB.set(&mut mcu, move || it_err.handler());
        I2cTask::new(dev)
    };

    // SPI ----------------------------------------------------------

    #[cfg(feature = "spi")]
    let pins = (gpioa.pa5, gpioa.pa6, gpioa.pa7);
    #[cfg(feature = "spi-it-sole")]
    let (dev, mut it, mut err_it) = dp.SPI1.init::<OS>(&mut mcu).into_interrupt_sole(
        pins,
        spi::MODE_0,
        200.kHz(),
        gpioa.pa4,
        0.nanos(),
        4,
        &mut mcu,
    );
    #[cfg(feature = "spi-it-bus")]
    let (dev, mut it, mut err_it) = {
        let (mut bus, it, err_it) = dp
            .SPI1
            .init::<OS>(&mut mcu)
            .into_interrupt_bus(pins, 4, &mut mcu);
        (
            bus.new_device(spi::MODE_0, 200.kHz(), gpioa.pa4, 0.nanos()),
            it,
            err_it,
        )
    };
    #[cfg(feature = "spi")]
    let mut spi_task = {
        its::SPI1_CB.set(&mut mcu, move || {
            it.handler();
            err_it.handler();
        });
        SpiTask::new(dev)
    };

    // PWM ----------------------------------------------------------

    #[cfg(feature = "timer")]
    {
        let c1 = gpioa.pa8;
        let mut tim1 = dp.TIM1.init(&mut mcu);
        tim1.set_count_direction(CountDirection::Up); // Optional
        let (mut bt, Some(mut ch1), None) =
            tim1.into_pwm2::<RemapDefault<_>>((c1, NONE_PIN), 20.kHz(), true, &mut mcu)
        else {
            panic!()
        };

        ch1.config(PwmMode::Mode1, PwmPolarity::ActiveHigh);
        ch1.set_duty_cycle(ch1.max_duty_cycle() / 2).ok();

        bt.start();
    }

    // External interrupt -------------------------------------------

    #[cfg(feature = "exti")]
    {
        let mut ex = gpiob.pb1.into_pull_up_input();
        ex.init_external_interrupt(Edge::Rising, true, &mut mcu.afio);
        its::EXTI1_CB.set(&mut mcu, move || {
            if ex.check_and_clear_interrupt() {
                static mut A: u32 = 0;
                unsafe {
                    A += 1;
                }
            }
        });
    }

    let mut loop_count: u32 = 0;
    let mut interval = Timeout::millis(500);
    run(move || {
        led_task.poll();
        #[cfg(feature = "uart")]
        uart_task.poll();
        #[cfg(feature = "i2c")]
        i2c_task.poll();
        #[cfg(feature = "spi")]
        spi_task.poll();

        if interval.timeout() {
            loop_count = loop_count.wrapping_add(1);
            l::info!("loop count: {}", loop_count);
            // if loop_count > 2 {
            //     l::panic!("loop");
            // }
        }
    })
}

mod its {
    use super::hal::interrupt_handler;
    interrupt_handler!(
        (USART1, USART1_CB),
        (EXTI1, EXTI1_CB),
        (DMA1_CHANNEL4, DMA1_CH4_CB),
        (DMA1_CHANNEL5, DMA1_CH5_CB),
        (I2C1_EV, I2C1_EVENT_CB),
        (I2C1_ER, I2C1_ERR_CB),
        (SPI1, SPI1_CB),
    );
}

#[allow(clippy::empty_loop)]
#[defmt::panic_handler]
fn defmt_panic() -> ! {
    cortex_m::interrupt::disable();
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    l::error!("{}", l::Display2Format(info));
    cortex_m::interrupt::disable();
    loop {}
}

#[allow(clippy::empty_loop)]
#[exception]
unsafe fn HardFault(ef: &ExceptionFrame) -> ! {
    l::error!("HardFault: {:#?}", l::Debug2Format(ef));
    loop {}
}

/// It's used to test Send trait
pub fn run<F>(mut func: F) -> !
where
    F: FnMut(),
    F: Send + 'static,
{
    loop {
        func();
    }
}
