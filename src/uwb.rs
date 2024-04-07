use dw3000_ng::hl::SendTime;
use esp_idf_svc::hal::gpio::{Gpio4, Gpio18, Gpio19, Gpio23, Gpio27, Gpio34};
use esp_idf_hal::sys::EspError;
use esp_idf_hal::gpio::{Input, InterruptType, PinDriver};
use esp_idf_hal::spi::config::{Mode, Phase, Polarity};
use esp_idf_hal::spi::{config, SpiDeviceDriver, SpiDriver, SpiDriverConfig, SPI3};
use std::sync::atomic::{AtomicBool, Ordering};
use esp_idf_svc::hal::prelude::*;
use std::sync::mpsc::{Receiver, SyncSender};
use dw3000_ng::{
    configs::{BitRate, Config, PreambleLength, PulseRepetitionFrequency, SfdSequence, StsLen, StsMode, UwbChannel},
    DW3000,
    // block,
};
use colored::*;

use ledswarm_protocol::Frame;

static WAS_INTERRUPT_TRIGGERED: AtomicBool = AtomicBool::new(false);

fn gpio_int_callback() {
    // Assert FLAG indicating a press button happened
    WAS_INTERRUPT_TRIGGERED.store(true, Ordering::Relaxed);
}

fn initialize_dw3000_interrupts(irq: Gpio34) -> PinDriver<'static, Gpio34, Input> {
    let mut dw3000_irq = PinDriver::input(irq).unwrap();
    dw3000_irq.set_interrupt_type(InterruptType::PosEdge).unwrap();
    unsafe { dw3000_irq.subscribe(gpio_int_callback).unwrap() }
    dw3000_irq.enable_interrupt().unwrap();

    dw3000_irq
}

fn reset_dw3000(rst: Gpio27) -> Result<(), EspError> {
    let delay = esp_idf_hal::delay::Delay::new_default();
    println!("--------->   DW3000 Reset");

    let mut rst_n = PinDriver::output(rst)?;
    rst_n.set_low().unwrap();
    delay.delay_ms(200);
    rst_n.set_high().unwrap();

    println!("--------->   Waiting for DW3000 to start up ... (5s)");

    // Time needed for DW3000 to start up (transition from INIT_RC to IDLE_RC, or could wait for SPIRDY event)
    delay.delay_ms(5000);

    Ok(())
}

/// Initialize the onboard ultra-wideband radio.
pub fn start(
    tx: SyncSender<Frame>,
    packet_rx: Receiver<Frame>,
    spi:        SPI3,
    serial_out: Gpio23,
    serial_in:  Gpio19,
    sclk:       Gpio18,
    cs:         Gpio4,
    irq:        Gpio34,
    rst:        Gpio27,
) -> Result<(), EspError> {
    let delay = esp_idf_hal::delay::Delay::new_default();

    let config = config::Config::new()
        .baudrate(5.MHz().into())
        .data_mode(Mode {
            polarity: Polarity::IdleLow,
            phase: Phase::CaptureOnFirstTransition,
        });

    let driver = SpiDriver::new::<SPI3>(
        spi,
        sclk,
        serial_out,
        Some(serial_in),
        &SpiDriverConfig::new(),
    )?;

    let spi_device = SpiDeviceDriver::new(driver, Some(cs), &config)?;
    println!("\n\n--------->   SPI initialized\n\n");

    let mut dw3000_irq = initialize_dw3000_interrupts(irq);
    reset_dw3000(rst);

    let dw3000_config = Config {
        channel: UwbChannel::Channel5,
        sfd_sequence: SfdSequence::Decawave8,
        pulse_repetition_frequency: PulseRepetitionFrequency::Mhz16,
        preamble_length: PreambleLength::Symbols1024,
        bitrate: BitRate::Kbps6800,
        frame_filtering: false,
        ranging_enable: true,
        sts_mode: StsMode::StsModeOff,
        sts_len: StsLen::StsLen64,
        sfd_timeout: 129,
    };
    let dw3000 = DW3000::new(spi_device)
		.init()
		.expect("Failed DWM3000 init.");
    let dw_res = dw3000.config(dw3000_config);

    
    match dw_res {
        Ok(mut uwb) => {
            println!("--------->   🎉  DWM3000 initialized");

            uwb.enable_rx_interrupts().expect("Failed to set up RX interrupts on the DW3000");

            loop {
                // See if there any packets to be sent
                if let Ok(packet) = packet_rx.try_recv() {
                    println!("## {}  Sending packet", "[uwb]".bright_blue().bold());
                    let packet_bytes = Vec::from(packet);
                    // Initiate Sending
                    let mut sending = uwb
                        .send(&packet_bytes[0 .. packet_bytes.len() - 4], SendTime::Now, Config::default())
                        .expect("Failed configure transmitter");

                    let send_result;

                    // Wait to send the frame, in a non-blocking way
                    loop {
                        if let Ok(t) = sending.s_wait() {
                            send_result = t;
                            break;
                        } else {
                            delay.delay_ms(1);
                        }
                    }
            
                    println!("Last frame sent at {}", send_result.value());
                    uwb = sending.finish_sending().expect("Failed to finish sending");
                    println!("## {}  Sent packet", "[uwb]".bright_blue().bold());
                }

                // Initiate Reception
                let mut buffer = [0; 1023];
                let mut receiving = uwb
                    .receive(Config::default())
                    .expect("Failed configure receiver.");
        
                // Waiting for an incoming frame
                if WAS_INTERRUPT_TRIGGERED.load(Ordering::Relaxed) {
                    // Reset global flag
                    WAS_INTERRUPT_TRIGGERED.store(false, Ordering::Relaxed);
                    // Re-enable the interrupt as it is disabled every time it is triggered
                    dw3000_irq.enable_interrupt().unwrap();
                    let result;

                    loop {
                        if let Ok(t) = receiving.r_wait(&mut buffer) {
                            result = t;
                            break;
                        } else {
                            delay.delay_ms(1);
                        }
                    }

                    let payload = result.frame.payload();

                    if let Some(bytes) = payload {
                        if let Ok(frame) = Frame::try_from(bytes.to_vec()) {
                            println!("## {}  Received packet: {:?}", "[uwb]".bright_blue().bold(), frame);
                            tx.send(frame).unwrap();
                        } else {
                            println!("Failed to parse UWB packet, skipping");
                        }
                    }
                } else {
                    delay.delay_ms(1);
                }

                // This must always execute at the end.
                uwb = receiving.finish_receiving().expect("Failed to finish receiving");
            }
        },
        Err(e) => println!("--------->  DW3000 config error: {:?}", e),
    }

    Ok(())
}