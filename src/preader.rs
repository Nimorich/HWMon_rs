// src/preader.rs
use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

use crate::channels::PackageSender;
use crate::uart::Uart;

/// Пакетный ридер для чтения данных с UART и формирования пакетов
/// Пакеты разделяются байтом-разделителем 0xC0
pub struct PReader {
    uart: Arc<Mutex<Uart>>,           // Защищенный доступ к UART
    package_sender: PackageSender,    // Канал для отправки собранных пакетов
    reading_active: bool,             // Флаг активности чтения
    current_packet: Vec<u8>,          // Текущий собираемый пакет
}

impl PReader {
    /// Создает новый экземпляр пакетного ридера
    pub fn new(uart: Arc<Mutex<Uart>>, package_sender: PackageSender) -> Self {
        Self {
            uart,
            package_sender,
            reading_active: false,
            current_packet: Vec::new(),
        }
    }

    /// Запускает процесс чтения данных с UART
    pub fn start_reading(&mut self) -> Result<()> {
        let uart_guard = self.uart.try_lock()
            .context("Failed to lock UART for reading")?;
        
        // Проверяем, что порт открыт перед началом чтения
        if !uart_guard.is_open() {
            return Err(anyhow::anyhow!("Serial port is not open"));
        }

        self.reading_active = true;
        self.current_packet.clear();
        println!("UART reading started");
        Ok(())
    }

    /// Останавливает процесс чтения данных
    pub fn stop_reading(&mut self) {
        self.reading_active = false;
        println!("UART reading stopped");
    }

    /// Возвращает статус активности чтения
    pub fn is_reading(&self) -> bool {
        self.reading_active
    }

    /// Основной цикл чтения данных с UART
    /// Асинхронно читает данные и обрабатывает их побайтово
    pub async fn read_loop(&mut self) -> Result<()> {
        if !self.reading_active {
            return Ok(());
        }

        loop {
            // Проверяем флаг активности на каждой итерации
            if !self.reading_active {
                break;
            }

            // Читаем новые данные с UART
            let new_data = {
                let mut uart_guard = self.uart.try_lock()
                    .context("Failed to lock UART for data reading")?;

                // Если данных нет, ждем немного и продолжаем
                if !uart_guard.is_readable() {
                    sleep(Duration::from_millis(10)).await;
                    continue;
                }

                uart_guard.read_all()?
            };

            // Обрабатываем полученные данные
            if !new_data.is_empty() {
                println!("UART read {} bytes", new_data.len());
                for byte in new_data {
                    self.process_byte(byte).await;
                }
            }

            // Небольшая пауза для снижения нагрузки на CPU
            sleep(Duration::from_millis(10)).await;
        }

        Ok(())
    }

    /// Обрабатывает отдельный байт данных
    /// Собирает пакет до встречи байта-разделителя 0xC0
    async fn process_byte(&mut self, byte: u8) {
        if !self.reading_active {
            return;
        }

        // Если байт не является разделителем, добавляем его в текущий пакет
        if byte != 0xC0 {
            self.current_packet.push(byte);
        }

        // Если встретили байт-разделитель, отправляем собранный пакет
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