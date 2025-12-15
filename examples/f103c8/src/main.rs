#![no_std]
#![no_main]
#![allow(unused)]

mod i2c_task;
mod led_task;
mod os;
mod uart_task;

use core::{mem::MaybeUninit, panic::PanicInfo};
use i2c_task::I2cTask;
use led_task::LedTask;
use os::*;
use uart_task::UartPollTask;

// Basic
use stm32f1_hal::{
    self as hal, Mcu, cortex_m::asm, cortex_m_rt::entry, gpio::PinState, i2c::I2cInit, pac, rcc,
};

use hal::{
    Heap,
    afio::{NONE_PIN, RemapDefault},
    dma::DmaPriority,
    embedded_hal::{self, pwm::SetDutyCycle},
    embedded_io,
    gpio::{Edge, ExtiPin},
    i2c,
    nvic_scb::PriorityGrouping,
    pac::Interrupt,
    time::MonoTimer,
    timer::{CountDirection, PwmMode, PwmPolarity},
    uart,
};

#[global_allocator]
static HEAP: Heap = Heap::empty();
const HEAP_SIZE: usize = 10 * 1024;
static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];

#[entry]
fn main() -> ! {
    let cp = cortex_m::Peripherals::take().unwrap();
    let mut scb = cp.SCB.init();
    // Set it as early as possible
    scb.set_priority_grouping(PriorityGrouping::Group4);
    // Initialize the heap BEFORE you use it
    unsafe { HEAP.init(&raw mut HEAP_MEM as usize, HEAP_SIZE) }

    // Clock --------------------------------------------------------
    let dp = pac::Peripherals::take().unwrap();
    let mut flash = dp.FLASH.init();
    let sysclk = 72.MHz();
    let cfg = rcc::Config::hse(8.MHz()).sysclk(sysclk);
    let mut rcc = dp.RCC.init().freeze(cfg, &mut flash.acr);
    assert_eq!(rcc.clocks.sysclk(), sysclk);

    let afio = dp.AFIO.init(&mut rcc);
    let mut mcu = Mcu::new(rcc, afio, scb, cp.NVIC.init(), dp.EXTI);

    let mut sys_timer = cp.SYST.counter_hz(&mcu);
    sys_timer.start(20.Hz()).unwrap();
    // let mono_timer = MonoTimer::new(cp.DWT, cp.DCB, &mcu.rcc.clocks);

    // Prepare ------------------------------------------------------

    // Keep them in one place for easier management
    mcu.nvic.disable_all(); // Optional
    mcu.nvic.set_priority(Interrupt::USART1, 2);
    mcu.nvic.set_priority(Interrupt::EXTI1, 1);
    mcu.nvic.set_priority(Interrupt::DMA1_CHANNEL4, 2);
    mcu.nvic.set_priority(Interrupt::DMA1_CHANNEL5, 2);
    mcu.nvic.set_priority(Interrupt::I2C1_EV, 3);
    mcu.nvic.set_priority(Interrupt::I2C1_ER, 3);

    let mut gpioa = dp.GPIOA.split(&mut mcu.rcc);
    let mut gpiob = dp.GPIOB.split(&mut mcu.rcc);
    let mut dma1 = dp.DMA1.split(&mut mcu.rcc);

    // LED ----------------------------------------------------------

    let led = gpiob
        .pb0
        .into_open_drain_output_with_state(&mut gpiob.crl, PinState::High);
    let mut led_task = LedTask::new(led);

    // UART ---------------------------------------------------------

    let pin_tx = gpioa.pa9.into_alternate_push_pull(&mut gpioa.crh);
    let pin_rx = gpioa.pa10.into_pull_up_input(&mut gpioa.crh);
    // let pin_tx = gpiob.pb6.into_alternate_push_pull(&mut gpiob.crl);
    // let pin_rx = gpiob.pb7.into_pull_up_input(&mut gpiob.crl);
    // let pin_rx = hal::afio::NONE_PIN;

    let config = uart::Config::default();
    let (Some(uart_tx), Some(uart_rx)) =
        dp.USART1
            .init::<OS>(&mut mcu)
            .into_tx_rx((pin_tx, pin_rx), config, &mut mcu)
    else {
        panic!()
    };

    #[cfg(feature = "uart_dma")]
    let (tx, rx) = {
        dma1.4.set_priority(DmaPriority::Medium);
        dma1.5.set_priority(DmaPriority::Medium);
        let (tx, mut tx_it) = uart_tx.into_dma_ringbuf(dma1.4, 32, 0.micros());
        let (rx, mut rx_it) = uart_rx.into_dma_circle(dma1.5, 64, 100.micros());
        all_it::DMA1_CH4_CB.set(&mut mcu, move || tx_it.interrupt_reload());
        all_it::DMA1_CH5_CB.set(&mut mcu, move || rx_it.interrupt_notify());
        (tx, rx)
    };

    #[cfg(feature = "uart_it")]
    let (tx, rx) = {
        let (tx, mut tx_it) = uart_tx.into_interrupt(32, 0.micros());
        let (rx, mut rx_it) = uart_rx.into_interrupt(64, 100.micros());
        all_it::USART1_CB.set(&mut mcu, move || {
            rx_it.handler();
            tx_it.handler();
        });
        (tx, rx)
    };

    #[cfg(feature = "uart_poll")]
    let (tx, rx) = (uart_tx.into_poll(0.micros()), uart_rx.into_poll(0.micros()));

    #[cfg(feature = "uart")]
    let mut uart_task = UartPollTask::new(32, tx, rx);

    // I2C ----------------------------------------------------------

    #[cfg(feature = "i2c_it")]
    let dev = {
        let scl = gpiob.pb6.into_alternate_open_drain(&mut gpiob.crl);
        let sda = gpiob.pb7.into_alternate_open_drain(&mut gpiob.crl);
        let (bus, mut it, mut it_err) = dp.I2C1.init::<OS>(&mut mcu).into_interrupt_bus(
            (scl, sda),
            i2c::Mode::from(200.kHz()),
            &mut mcu,
        );
        all_it::I2C1_EVENT_CB.set(&mut mcu, move || it.handler());
        all_it::I2C1_ERR_CB.set(&mut mcu, move || it_err.handler());
        bus.new_device(i2c::Address::Seven(0b1101000))
    };

    #[cfg(feature = "i2c")]
    let mut i2c_task = I2cTask::new(dev);

    // PWM ----------------------------------------------------------

    #[cfg(feature = "timer")]
    {
        let c1 = gpioa.pa8.into_alternate_push_pull(&mut gpioa.crh);
        let mut tim1 = dp.TIM1.init(&mut mcu);
        tim1.set_count_direction(CountDirection::Up); // Optional
        let (mut bt, Some(mut ch1), _) =
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
        let mut ex = gpiob.pb1.into_pull_up_input(&mut gpiob.crl);
        ex.make_interrupt_source(&mut mcu.afio);
        ex.trigger_on_edge(Edge::Rising);
        ex.enable_interrupt();
        all_it::EXTI1_CB.set(&mut mcu, move || {
            if ex.check_interrupt() {
                ex.clear_interrupt_pending_bit();
            }
        });
    }

    loop {
        led_task.poll();
        #[cfg(feature = "uart")]
        uart_task.poll();
        #[cfg(feature = "i2c")]
        i2c_task.poll();
    }
}

mod all_it {
    use super::hal::interrupt_handler;
    interrupt_handler!(
        (USART1, USART1_CB),
        (EXTI1, EXTI1_CB),
        (DMA1_CHANNEL4, DMA1_CH4_CB),
        (DMA1_CHANNEL5, DMA1_CH5_CB),
        (I2C1_EV, I2C1_EVENT_CB),
        (I2C1_ER, I2C1_ERR_CB),
    );
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    asm::bkpt();
    loop {}
}
