// src/preader.rs
use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

use crate::channels::PackageSender;
use crate::uart::Uart;

pub struct PReader {
    uart: Arc<Mutex<Uart>>,
    package_sender: PackageSender,
    reading_active: bool,
    current_packet: Vec<u8>,
}

impl PReader {
    pub fn new(uart: Arc<Mutex<Uart>>, package_sender: PackageSender) -> Self {
        Self {
            uart,
            package_sender,
            reading_active: false,
            current_packet: Vec::new(),
        }
    }

    pub fn start_reading(&mut self) -> Result<()> {
        let uart_guard = self.uart.try_lock()
            .context("Failed to lock UART for reading")?;
        
        if !uart_guard.is_open() {
            return Err(anyhow::anyhow!("Serial port is not open"));
        }

        self.reading_active = true;
        self.current_packet.clear();
        println!("UART reading started");
        Ok(())
    }

    pub fn stop_reading(&mut self) {
        self.reading_active = false;
        println!("UART reading stopped");
    }

    pub fn is_reading(&self) -> bool {
        self.reading_active
    }

    pub async fn read_loop(&mut self) -> Result<()> {
        if !self.reading_active {
            return Ok(());
        }

        loop {
            if !self.reading_active {
                break;
            }

            let new_data = {
                let mut uart_guard = self.uart.try_lock()
                    .context("Failed to lock UART for data reading")?;

                if !uart_guard.is_readable() {
                    sleep(Duration::from_millis(10)).await;
                    continue;
                }

                uart_guard.read_all()?
            };

            if !new_data.is_empty() {
                println!("UART read {} bytes", new_data.len());
                for byte in new_data {
                    self.process_byte(byte).await;
                }
            }

            sleep(Duration::from_millis(10)).await;
        }

        Ok(())
    }

    async fn process_byte(&mut self, byte: u8) {
        if !self.reading_active {
            return;
        }

        if byte != 0xC0 {
            self.current_packet.push(byte);
        }

        if byte == 0xC0 {
            if !self.current_packet.is_empty() {
                // Отправляем пакет через канал (аналог signalPRPackage)
                if let Err(e) = self.package_sender.send(self.current_packet.clone()) {
                    eprintln!("Failed to send UART package: {}", e);
                } else {
                    println!("UART packet sent: {}", hex::encode(&self.current_packet));
                }
                self.current_packet.clear();
            }
        }
    }
}