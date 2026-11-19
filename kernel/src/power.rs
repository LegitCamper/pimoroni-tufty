use embassy_rp::{
    Peri,
    gpio::{Input, Pull},
    pac,
    peripherals::PIN_14,
};
use embassy_time::{Instant, Timer};
use tufty_sys::buttons::ResetHold;

const PASSWORD: u32 = 0x5afe_0000;

pub async fn check_reset(pin: Peri<'static, PIN_14>) {
    for channel in 0..4 {
        pac::POWMAN.pwrup(channel).write(|control| {
            control.0 = PASSWORD;
            control.set_status(true);
        });
    }
    pac::POWMAN.timer().write(|control| {
        control.0 = PASSWORD;
        control.set_alarm(true);
    });
    pac::POWMAN
        .dbg_pwrcfg()
        .write(|control| control.0 = PASSWORD);
    let reset = Input::new(pin, Pull::Up);
    let mut hold = ResetHold::default();
    loop {
        let pressed = reset.is_low();
        if hold.sample(pressed, Instant::now().as_millis()) {
            sleep();
        }
        if !pressed {
            return;
        }
        Timer::after_millis(10).await;
    }
}

// ponytail: reset-time sleep only, before core1/drivers start; wake cold-boots, not resumes.
fn sleep() -> ! {
    for bank in 0..2 {
        pac::SIO.gpio_oe(bank).value_clr().write_value(u32::MAX);
        pac::SIO.gpio_out(bank).value_clr().write_value(u32::MAX);
    }
    for pin in 0..48 {
        let pull_up = matches!(pin, 6 | 7 | 8 | 9 | 10 | 11 | 15);
        let floating = matches!(pin, 14 | 22 | 40 | 41 | 42);
        pac::IO_BANK0
            .gpio(pin)
            .ctrl()
            .write(|control| control.set_funcsel(5));
        pac::PADS_BANK0.gpio(pin).modify(|pad| {
            pad.set_ie(pin == 15);
            pad.set_od(false);
            pad.set_iso(false);
            pad.set_pue(pull_up);
            pad.set_pde(!pull_up && !floating);
        });
    }
    pac::SIO
        .gpio_oe(0)
        .value_set()
        .write_value((1 << 23) | (1 << 26));
    pac::SIO.gpio_oe(1).value_set().write_value(1 << (41 - 32));

    pac::RESETS.reset().modify(|reset| reset.set_usbctrl(true));
    pac::RESETS.reset().modify(|reset| reset.set_usbctrl(false));
    while !pac::RESETS.reset_done().read().usbctrl() {}
    pac::USB.usb_muxing().write(|control| {
        control.set_to_phy(true);
        control.set_softcon(true);
    });
    pac::USB
        .main_ctrl()
        .write(|control| control.set_controller_en(true));
    pac::USB.usbphy_direct().write(|control| {
        control.set_tx_pd(true);
        control.set_rx_pd(true);
        control.set_dp_pulldn_en(true);
        control.set_dm_pulldn_en(true);
    });
    pac::USB.usbphy_direct_override().write(|control| {
        control.set_tx_pd_override_en(true);
        control.set_rx_pd_override_en(true);
        control.set_tx_dp_oe_override_en(true);
        control.set_tx_dm_oe_override_en(true);
        control.set_dp_pullup_en_override_en(true);
        control.set_dm_pullup_override_en(true);
        control.set_dp_pulldn_en_override_en(true);
        control.set_dm_pulldn_en_override_en(true);
    });

    pac::POWMAN.pwrup(3).write(|control| {
        control.0 = PASSWORD;
        control.set_source(15);
        control.set_mode(pac::powman::vals::Mode::EDGE);
        control.set_direction(pac::powman::vals::Direction::LOW_FALLING);
        control.set_status(true);
    });
    pac::POWMAN.pwrup(3).modify(|control| {
        control.0 = PASSWORD | (control.0 & 0xffff);
        control.set_status(false);
        control.set_enable(true);
    });
    pac::POWMAN.vreg_ctrl().modify(|control| {
        control.0 = PASSWORD | (control.0 & 0xffff);
        control.set_unlock(true);
    });
    pac::POWMAN.seq_cfg().modify(|control| {
        control.0 = PASSWORD | (control.0 & 0xffff);
        control.set_hw_pwrup_sram0(false);
        control.set_hw_pwrup_sram1(false);
        control.set_use_vreg_lp(true);
        control.set_use_vreg_hp(true);
        control.set_run_lposc_in_lp(true);
    });
    pac::POWMAN.dbg_pwrcfg().write(|control| {
        control.0 = PASSWORD;
        control.set_ignore(true);
    });
    for index in 0..4 {
        pac::POWMAN.boot(index).write_value(0);
    }

    cortex_m::interrupt::disable();
    // SAFETY: reset-time terminal path; no other task/core uses these core peripherals.
    let mut core = unsafe { cortex_m::Peripherals::steal() };
    core.SYST.disable_counter();
    core.SYST.disable_interrupt();
    for register in &core.NVIC.icer[..2] {
        // SAFETY: only the two implemented NVIC banks are written; reboot restores IRQs.
        unsafe { register.write(u32::MAX) };
    }
    cortex_m::peripheral::SCB::clear_pendst();
    cortex_m::peripheral::SCB::clear_pendsv();
    power_down();
}

#[unsafe(link_section = ".data")]
#[inline(never)]
fn power_down() -> ! {
    pac::POWMAN.state().write(|control| {
        control.0 = PASSWORD;
        control.set_req_ignored(true);
        control.set_pwrup_while_waiting(true);
        control.set_req(0xf);
    });
    for _ in 0..100_000 {
        let state = pac::POWMAN.state().read();
        if state.req_ignored() || state.bad_sw_req() || state.bad_hw_req() {
            break;
        }
        if state.waiting() {
            cortex_m::asm::dsb();
            cortex_m::asm::wfi();
            break;
        }
    }
    pac::POWMAN.state().write(|control| control.0 = PASSWORD);
    cortex_m::peripheral::SCB::sys_reset();
}
