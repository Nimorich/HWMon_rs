use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::channels::CommandSender;
use crate::uart::Uart;

/// Структура для записи команд и данных через UART
pub struct PWriter {
    uart: Arc<Mutex<Uart>>,           // Защищенный доступ к UART
    command_sender: CommandSender,    // Канал для отправки команд
}

impl PWriter {
    /// Создает новый экземпляр писателя пакетов
    pub fn new(uart: Arc<Mutex<Uart>>, command_sender: CommandSender) -> Self {
        println!("PWriter initialized with UART");
        Self { uart, command_sender }
    }

    /// Записывает команду непосредственно в UART
    pub fn write_command(&self, command: &[u8]) -> Result<()> {
        if command.is_empty() {
            return Err(anyhow::anyhow!("Empty command received"));
        }

        println!("PWriter: Preparing to write command, size: {}", command.len());

        // Получаем доступ к UART
        let mut uart_guard = self.uart.try_lock()
            .context("Failed to lock UART for writing")?;

        // Проверяем, что порт открыт
        if !uart_guard.is_open() {
            return Err(anyhow::anyhow!("Serial port is not open for writing"));
        }

        // Записываем команду в порт
        uart_guard.write_all(command)?;
        println!("Successfully written {} bytes to UART", command.len());
        println!("Command data: {}", hex::encode(command));

        Ok(())
    }

    /// Отправляет команду через канал для асинхронной обработки
    pub fn send_command(&self, command: Vec<u8>) -> Result<()> {
        self.command_sender.send(command)
            .map_err(|e| anyhow::anyhow!("Failed to send command: {}", e))
    }
}