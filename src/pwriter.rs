use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::channels::CommandSender;
use crate::uart::Uart;

pub struct PWriter {
    uart: Arc<Mutex<Uart>>,
    command_sender: CommandSender,
}

impl PWriter {
    pub fn new(uart: Arc<Mutex<Uart>>, command_sender: CommandSender) -> Self {
        println!("PWriter initialized with UART");
        Self { uart, command_sender }
    }

    pub fn write_command(&self, command: &[u8]) -> Result<()> {
        if command.is_empty() {
            return Err(anyhow::anyhow!("Empty command received"));
        }

        println!("PWriter: Preparing to write command, size: {}", command.len());

        let mut uart_guard = self.uart.try_lock()
            .context("Failed to lock UART for writing")?;

        if !uart_guard.is_open() {
            return Err(anyhow::anyhow!("Serial port is not open for writing"));
        }

        uart_guard.write_all(command)?;
        println!("Successfully written {} bytes to UART", command.len());
        println!("Command data: {}", hex::encode(command));

        Ok(())
    }

    // Метод для отправки команд через канал
    pub fn send_command(&self, command: Vec<u8>) -> Result<()> {
        self.command_sender.send(command)
            .map_err(|e| anyhow::anyhow!("Failed to send command: {}", e))
    }
}