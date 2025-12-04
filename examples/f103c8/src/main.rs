#![no_std]
#![no_main]
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_mut)]

use core::{mem::MaybeUninit, panic::PanicInfo};
use stm32f1_hal::{
    self as hal, Heap, Mcu, Steal,
    afio::{NONE_PIN, RemapDefault},
    cortex_m::asm,
    cortex_m_rt::entry,
    dma::{DmaBindRx, DmaBindTx, DmaEvent, DmaPriority},
    embedded_hal::{self, pwm::SetDutyCycle},
    embedded_io,
    gpio::{Edge, ExtiPin, PinState},
    nvic_scb::PriorityGrouping,
    pac::{self, Interrupt},
    prelude::*,
    rcc,
    time::MonoTimer,
    timer::*,
    uart::{self, UartPeriphExt},
    waiter_trait::{self, Counter, prelude::*},
};

mod led_task;
use led_task::LedTask;
mod uart_task;
use uart_task::UartPollTask;

#[global_allocator]
static HEAP: Heap = Heap::empty();
const HEAP_SIZE: usize = 10 * 1024;
static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];

#[entry]
fn main() -> ! {
    let cp = cortex_m::Peripherals::take().unwrap();
    let mut scb = cp.SCB.constrain();
    // Set it as early as possible
    scb.set_priority_grouping(PriorityGrouping::Group4);
    // Initialize the heap BEFORE you use it
    unsafe { HEAP.init(&raw mut HEAP_MEM as usize, HEAP_SIZE) }

    let dp = pac::Peripherals::take().unwrap();
    let mut flash = dp.FLASH.constrain();
    let sysclk = 72.MHz();
    let cfg = rcc::Config::hse(8.MHz()).sysclk(sysclk);
    let mut rcc = dp.RCC.constrain().freeze(cfg, &mut flash.acr);
    assert_eq!(rcc.clocks.sysclk(), sysclk);

    let afio = dp.AFIO.constrain(&mut rcc);
    let mut mcu = Mcu {
        scb,
        nvic: cp.NVIC.constrain(),
        rcc,
        afio,
        exti: dp.EXTI,
    };

    let mut sys_timer = cp.SYST.counter_hz(&mcu);
    sys_timer.start(20.Hz()).unwrap();
    let mono_timer = MonoTimer::new(cp.DWT, cp.DCB, &mcu.rcc.clocks);

    // Keep them in one place for easier management
    mcu.nvic.enable(Interrupt::USART1, false); // Optional
    mcu.nvic.set_priority(Interrupt::USART1, 2);
    mcu.nvic.enable(Interrupt::EXTI1, false);
    mcu.nvic.set_priority(Interrupt::EXTI1, 1);
    mcu.nvic.enable(Interrupt::DMA1_CHANNEL4, false);
    mcu.nvic.set_priority(Interrupt::DMA1_CHANNEL4, 2);

    let mut gpioa = dp.GPIOA.split(&mut mcu.rcc);
    let mut gpiob = dp.GPIOB.split(&mut mcu.rcc);
    let mut dma1 = dp.DMA1.split(&mut mcu.rcc);

    // UART -------------------------------------

    // let pin_tx = gpioa.pa9.into_alternate_push_pull(&mut gpioa.crh);
    // let pin_rx = gpioa.pa10.into_pull_up_input(&mut gpioa.crh);
    let pin_tx = gpiob.pb6.into_alternate_push_pull(&mut gpiob.crl);
    let pin_rx = gpiob.pb7.into_pull_up_input(&mut gpiob.crl);
    // let pin_rx = hal::afio::NONE_PIN;

    let config = uart::Config::default();
    let (Some(uart_tx), Some(uart_rx)) =
        dp.USART1
            .constrain(&mut mcu)
            .into_tx_rx((pin_tx, pin_rx), config, &mut mcu)
    else {
        panic!()
    };

    // let mut uart_task = uart_poll_init(uart_tx, uart_rx);
    // let mut uart_task =
    //     uart_interrupt_init(uart_tx, uart_rx, &all_it::USART1_CB, &mut mcu, &sys_timer);
    dma1.4.set_priority(DmaPriority::Medium);
    dma1.5.set_priority(DmaPriority::Medium);
    let mut uart_task = uart_dma_init(
        uart_tx,
        dma1.4,
        &all_it::DMA1_CHANNEL4_CB,
        uart_rx,
        dma1.5,
        &mut mcu,
        &sys_timer,
    );

    // LED --------------------------------------

    let mut led = gpiob
        .pb0
        .into_open_drain_output_with_state(&mut gpiob.crl, PinState::High);
    let mut water = sys_timer.waiter(1.secs());
    // let water = mono_timer.waiter(1.secs());
    let mut led_task = LedTask::new(led, water.start());

    // PWM --------------------------------------

    let c1 = gpioa.pa8.into_alternate_push_pull(&mut gpioa.crh);
    let mut tim1 = dp.TIM1.constrain(&mut mcu);
    tim1.set_count_direction(CountDirection::Up); // Optional
    let (mut bt, Some(mut ch1), _) =
        tim1.into_pwm2::<RemapDefault<_>>((c1, NONE_PIN), 20.kHz(), true, &mut mcu)
    else {
        panic!()
    };

    ch1.config(PwmMode::Mode1, PwmPolarity::ActiveHigh);
    ch1.set_duty_cycle(ch1.max_duty_cycle() / 2).ok();

    bt.start();

    // External interruption --------------------

    let mut ex = gpiob.pb1.into_pull_up_input(&mut gpiob.crl);
    let ex1_ctl = unsafe { ex.steal() }; // Optional
    ex.make_interrupt_source(&mut mcu.afio);
    ex.trigger_on_edge(Edge::Rising);
    ex.enable_interrupt();
    all_it::EXTI1_CB.set(&mut mcu, move || {
        if ex.check_interrupt() {
            ex.clear_interrupt_pending_bit();
        }
    });

    loop {
        led_task.poll();
        uart_task.poll();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    asm::bkpt();
    loop {}
}

fn uart_poll_init<U: UartPeriphExt>(
    tx: uart::Tx<U>,
    rx: uart::Rx<U>,
) -> UartPollTask<impl embedded_io::Write, impl embedded_io::Read> {
    let uart_rx = rx.into_poll(Counter::new(0), Counter::new(1_000));
    let uart_tx = tx.into_poll(Counter::new(0), Counter::new(10_000));
    UartPollTask::new(32, uart_tx, uart_rx)
}

fn uart_interrupt_init<U: UartPeriphExt + 'static>(
    tx: uart::Tx<U>,
    rx: uart::Rx<U>,
    interrupt_callback: &hal::interrupt::Callback,
    mcu: &mut Mcu,
    timer: &SystemTimer,
) -> UartPollTask<impl embedded_io::Write + 'static, impl embedded_io::Read + 'static> {
    let (rx, mut rx_it) = rx.into_interrupt(64, timer.waiter(100.micros()));
    let (tx, mut tx_it) = tx.into_interrupt(
        32,
        timer.waiter(0.micros()),
        timer.waiter(32 * 200.micros()),
    );
    interrupt_callback.set(mcu, move || {
        rx_it.handler();
        tx_it.handler();
    });
    UartPollTask::new(32, tx, rx)
}

fn uart_dma_init<'r, U: UartPeriphExt + 'static>(
    tx: uart::Tx<U>,
    mut dma_tx: impl DmaBindTx<U> + 'static,
    interrupt_callback: &hal::interrupt::Callback,
    rx: uart::Rx<U>,
    dma_rx: impl DmaBindRx<U> + 'r,
    mcu: &mut Mcu,
    timer: &SystemTimer,
) -> UartPollTask<impl embedded_io::Write + 'static, impl embedded_io::Read + 'r> {
    let uart_rx = rx.into_dma_circle(dma_rx, 64, timer.waiter(100.micros()));
    dma_tx.set_interrupt(DmaEvent::TransferComplete, true);
    let (uart_tx, mut tx_it) = tx.into_dma_ringbuf(
        dma_tx,
        32,
        timer.waiter(0.micros()),
        timer.waiter(32 * 200.micros()),
    );
    interrupt_callback.set(mcu, move || {
        tx_it.interrupt_reload();
    });
    UartPollTask::new(32, uart_tx, uart_rx)
}

mod all_it {
    use super::hal::{interrupt_handler, pac::interrupt};
    interrupt_handler!(
        (USART1, USART1_CB),
        (EXTI1, EXTI1_CB),
        (DMA1_CHANNEL4, DMA1_CHANNEL4_CB),
    );
}
