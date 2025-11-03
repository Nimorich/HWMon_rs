use anyhow::{Context, Result};
use clap::{Arg, Command};
use std::process;
use tokio;

// Модули приложения
mod channels;
mod controller;
mod dump_reader;
mod preader;
mod psorter;
mod pwriter;
mod zmq_sender;
mod uart;

// Вспомогательные модули для обработки данных
mod include {
    pub mod byte_stuffing;
    pub mod crc;
    pub mod linear11;
}

use controller::Controller;

/// Главная функция приложения HWMon
/// Управляет работой монитора оборудования через различные интерфейсы
#[tokio::main]
async fn main() -> Result<()> {
    // Парсинг аргументов командной строки
    let matches = Command::new("HWMon")
        .about("Hardware Monitor Application")
        .arg(
            Arg::new("operation")
                .required(true)
                .index(1)
                .help("Operation mode: DUMP <filename> / UART / CAN")
        )
        .arg(
            Arg::new("filename")
                .required(false)
                .index(2)
                .help("Dump filename for DUMP mode")
        )
        .get_matches();

    let operation = matches.get_one::<String>("operation")
        .context("Operation argument is required")?;

    // Создаем контроллер приложения
    let mut controller = Controller::new().await?;

    // Обработка различных режимов работы
    match operation.as_str() {
        "DUMP" => {
            // Режим обработки файла дампа
            let filename = matches.get_one::<String>("filename")
                .context("Filename required for DUMP mode")?;
            controller.set_read_operation(controller::ReadOperation::Dump);
            controller.set_dump_filename(filename.clone());
            
            // Запускаем обработку
            controller.start().await?;
            println!("Processing dump file...");
            
            // Ждем завершения обработки дампа
            controller.wait_for_completion().await;
            println!("Dump processing finished");
            
            // Выводим статистику сразу после завершения
            controller.print_statistics().await;
        }
        "UART" => {
            // Режим непрерывного чтения с UART
            controller.set_read_operation(controller::ReadOperation::Uart);
            controller.start().await?;
            
            println!("UART mode started - reading continuously. Press Ctrl+C to stop");
            tokio::signal::ctrl_c().await?;
            println!("Shutting down UART mode...");
            
            // Выводим статистику для UART режима
            controller.print_statistics().await;
        }
        "CAN" => {
            // Режим CAN (пока не реализован)
            controller.set_read_operation(controller::ReadOperation::Can);
            println!("CAN mode selected - not implemented yet");
            
            // Выводим пустую статистику для CAN режима
            controller.print_statistics().await;
        }
        _ => {
            // Неизвестный режим работы
            eprintln!("Unknown operation: {}", operation);
            eprintln!("Use: DUMP <filename> / UART / CAN");
            process::exit(1);
        }
    }

    Ok(())
}