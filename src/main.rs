use anyhow::{Context, Result};
use clap::{Arg, Command};
use std::process;
use tokio;

mod channels;
mod controller;
mod dump_reader;
mod preader;
mod psorter;
mod pwriter;
mod zmq_sender;
mod uart;


mod include {
    pub mod byte_stuffing;
    pub mod crc;
    pub mod linear11;
}

use controller::Controller;

#[tokio::main]
async fn main() -> Result<()> {
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

    let mut controller = Controller::new().await?;

    match operation.as_str() {
        "DUMP" => {
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
            controller.set_read_operation(controller::ReadOperation::Uart);
            controller.start().await?;
            
            println!("UART mode started - reading continuously. Press Ctrl+C to stop");
            tokio::signal::ctrl_c().await?;
            println!("Shutting down UART mode...");
            
            // Выводим статистику для UART режима
            controller.print_statistics().await;
        }
        "CAN" => {
            controller.set_read_operation(controller::ReadOperation::Can);
            println!("CAN mode selected - not implemented yet");
            
            // Выводим пустую статистику для CAN режима
            controller.print_statistics().await;
        }
        _ => {
            eprintln!("Unknown operation: {}", operation);
            eprintln!("Use: DUMP <filename> / UART / CAN");
            process::exit(1);
        }
    }

    Ok(())
}