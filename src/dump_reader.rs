// src/dump_reader.rs
use anyhow::{Context, Result};
use std::fs::File;
use std::io::Read;
use tokio::time::{sleep, Duration};
use crate::channels::PackageSender;

pub struct DumpReader {
    send_package_counter: u32,
    package_sender: PackageSender,
}

impl DumpReader {
    pub fn new(package_sender: PackageSender) -> Self {
        Self {
            send_package_counter: 0,
            package_sender,
        }
    }

    pub async fn start_read(mut self, filename: &str) -> Result<()> {
        let mut file = File::open(filename)
            .context(format!("Failed to open dump file: {}", filename))?;
        
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .context("Failed to read dump file")?;

        println!("Dump file loaded, size: {} bytes", buffer.len());
        
        // Обрабатываем байты потоково
        let mut current_packet = Vec::new();
        
        for (i, &byte) in buffer.iter().enumerate() {                        
            // Добавляем байт в текущий пакет
            if byte != 0xC0 {
                current_packet.push(byte);
            }

            // Проверяем, является ли этот байт разделителем 0xC0
            if byte == 0xC0 {
                // Отправляем готовый пакет через канал
                if !current_packet.is_empty() {
                    if let Err(e) = self.package_sender.send(current_packet.clone()) {
                        eprintln!("Failed to send dump package: {}", e);
                    } else {
                        if self.send_package_counter % 100 == 0 { // Логируем только каждые 100 пакетов
                            println!("Dump packet {} sent", self.send_package_counter + 1);
                        }
                        self.send_package_counter += 1;
                    }
                    // Начинаем новый пакет
                    current_packet.clear();
                }
            }
        }
        
        // Отправляем последний пакет, если он есть
        if !current_packet.is_empty() {
            if let Err(e) = self.package_sender.send(current_packet) {
                eprintln!("Failed to send final dump package: {}", e);
            } else {
                println!("Final dump packet {} sent", self.send_package_counter + 1);
                self.send_package_counter += 1;
            }
        }
        
        println!("Dump processing completed. Total packets sent: {}", self.send_package_counter);
        
        // Канал автоматически закроется когда DumpReader выйдет из области видимости
        Ok(())
    }
}