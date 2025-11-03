use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::channels::{PackageSender, CommandSender, package_channel, command_channel};
use crate::dump_reader::DumpReader;
use crate::preader::PReader;
use crate::psorter::PSorter;
use crate::pwriter::PWriter;
use crate::uart::Uart;
use crate::zmq_sender::ZmqSender;

#[derive(Debug, Clone, PartialEq)]
pub enum ReadOperation {
    Uart,
    Dump,
    Can,
}

pub struct Controller {
    // ZMQ отправители
    sender_t: Arc<ZmqSender>,
    sender_s: Arc<ZmqSender>,
    sender_pu: Arc<ZmqSender>,
    sender_o: Arc<ZmqSender>,
    sender_c: Arc<ZmqSender>,
    
    // Каналы
    package_sender: PackageSender,
    command_sender: CommandSender,
    
    // Компоненты
    dump_reader: Option<DumpReader>,
    uart: Option<Arc<Mutex<Uart>>>,
    p_reader: Option<Arc<Mutex<PReader>>>,
    p_writer: Option<Arc<Mutex<PWriter>>>,
    p_sorter: Arc<Mutex<PSorter>>,
    
    // Состояние
    dump_filename: Option<String>,
    read_operation: ReadOperation,
    
    // Асинхронные задачи
    tasks: Vec<JoinHandle<()>>,
}

impl Controller {
    pub async fn new() -> Result<Self> {
        let sender_t = Arc::new(ZmqSender::new("tcp://*:5555"));
        let sender_s = Arc::new(ZmqSender::new("tcp://*:5556"));
        let sender_pu = Arc::new(ZmqSender::new("tcp://*:5557"));
        let sender_o = Arc::new(ZmqSender::new("tcp://*:5558"));
        let sender_c = Arc::new(ZmqSender::new("tcp://*:5559"));

        let (package_sender, package_receiver) = package_channel();
        let (command_sender, command_receiver) = command_channel();

        // Запускаем обработчик пакетов
        let sorter = Arc::new(Mutex::new(PSorter::new()));
        let sorter_clone = Arc::clone(&sorter);
        
        // Создаем клоны для использования в задаче
        let sender_t_clone = Arc::clone(&sender_t);
        let sender_s_clone = Arc::clone(&sender_s);
        let sender_pu_clone = Arc::clone(&sender_pu);
        let sender_o_clone = Arc::clone(&sender_o);
        let sender_c_clone = Arc::clone(&sender_c);

        let package_handler = tokio::spawn(async move {
            Self::handle_packages(
                package_receiver,
                sorter_clone,
                sender_t_clone,
                sender_s_clone,
                sender_pu_clone,
                sender_o_clone,
                sender_c_clone,
            ).await;
        });

        // Запускаем обработчик команд
        let command_handler = tokio::spawn(async move {
            Self::handle_commands(command_receiver).await;
        });

        Ok(Self {
            sender_t,
            sender_s,
            sender_pu,
            sender_o,
            sender_c,
            package_sender,
            command_sender,
            dump_reader: None,
            uart: None,
            p_reader: None,
            p_writer: None,
            p_sorter: sorter,
            dump_filename: None,
            read_operation: ReadOperation::Uart,
            tasks: vec![package_handler, command_handler],
        })
    }

    pub fn set_read_operation(&mut self, operation: ReadOperation) {
        self.read_operation = operation.clone();
        println!("Read operation changed to: {:?}", operation);
    }

    pub fn set_dump_filename(&mut self, filename: String) {
        self.dump_filename = Some(filename);
    }

    pub async fn start(&mut self) -> Result<()> {
        println!("================================================");
        println!("Starting server with ZeroMQ endpoints:");
        println!("TMonitor:  tcp://localhost:5555");
        println!("SMonitor:  tcp://localhost:5556");
        println!("PUMonitor: tcp://localhost:5557");
        println!("OMonitor:  tcp://localhost:5558");
        println!("CMonitor:  tcp://localhost:5559");
        println!("Current read operation: {:?}", self.read_operation);
        println!("================================================");

        match self.read_operation {
            ReadOperation::Uart => {
                self.start_uart_mode().await?;
            }
            ReadOperation::Dump => {
                self.start_dump_mode().await?;
            }
            ReadOperation::Can => {
                println!("CAN mode selected - not implemented yet");
            }
        }

        Ok(())
    }

    async fn start_uart_mode(&mut self) -> Result<()> {
        let uart = Arc::new(Mutex::new(Uart::new()?));
        let mut p_reader = PReader::new(
            Arc::clone(&uart),
            self.package_sender.clone(),
        );
        
        // Запускаем чтение из UART в отдельной задаче
        let uart_task = tokio::spawn(async move {
            if let Err(e) = p_reader.start_reading() {
                eprintln!("Failed to start UART reading: {}", e);
                return;
            }
            
            if let Err(e) = p_reader.read_loop().await {
                eprintln!("UART reading error: {}", e);
            }
        });
        
        self.uart = Some(uart);
        self.tasks.push(uart_task);
        
        println!("UART mode started - reading continuously");
        Ok(())
    }

    async fn start_dump_mode(&mut self) -> Result<()> {
        let filename = self.dump_filename.as_ref()
            .context("Dump filename not set")?;
        
        let dump_reader = DumpReader::new(self.package_sender.clone()); // убираем mut
        let filename_clone = filename.clone();
        let dump_task = tokio::spawn(async move {
            if let Err(e) = dump_reader.start_read(&filename_clone).await {
                eprintln!("Dump reading error: {}", e);
            } else {
                println!("Dump processing task completed");
            }
            // Когда задача завершается, package_sender выходит из области видимости
            // и канал автоматически закрывается
        });
        self.tasks.push(dump_task);
        
        println!("Dump mode started - streaming packets");
        Ok(())
    }

    async fn handle_packages(
        mut package_receiver: crate::channels::PackageReceiver,
        sorter: Arc<Mutex<PSorter>>,
        sender_t: Arc<ZmqSender>,
        sender_s: Arc<ZmqSender>,
        sender_pu: Arc<ZmqSender>,
        sender_o: Arc<ZmqSender>,
        sender_c: Arc<ZmqSender>,
    ) {
        while let Some(package) = package_receiver.recv().await {
            let mut sorter_guard = sorter.lock().await;
            
            // Создаем клоны для замыкания
            let sender_t = Arc::clone(&sender_t);
            let sender_s = Arc::clone(&sender_s);
            let sender_pu = Arc::clone(&sender_pu);
            let sender_o = Arc::clone(&sender_o);
            let sender_c = Arc::clone(&sender_c);
            
            sorter_guard.slot_input_package(&package, move |pack_type, data| {
                match pack_type {
                    1 => {
                        if let Err(e) = sender_t.send_package(data) {
                            eprintln!("Failed to send to TMonitor: {}", e);
                        } else {
                            println!("Successfully sent to TMonitor");
                        }
                    }
                    2 => {
                        if let Err(e) = sender_s.send_package(data) {
                            eprintln!("Failed to send to SMonitor: {}", e);
                        } else {
                            println!("Successfully sent to SMonitor");
                        }
                    }
                    3 => {
                        if let Err(e) = sender_pu.send_package(data) {
                            eprintln!("Failed to send to PUMonitor: {}", e);
                        } else {
                            println!("Successfully sent to PUMonitor");
                        }
                    }
                    4 => {
                        if let Err(e) = sender_o.send_package(data) {
                            eprintln!("Failed to send to OMonitor: {}", e);
                        } else {
                            println!("Successfully sent to OMonitor");
                        }
                    }
                    5 => {
                        if let Err(e) = sender_c.send_package(data) {
                            eprintln!("Failed to send to CMonitor: {}", e);
                        } else {
                            println!("Successfully sent to CMonitor");
                        }
                    }
                    _ => {
                        if let Err(e) = sender_o.send_package(data) {
                            eprintln!("Failed to send to OMonitor: {}", e);
                        } else {
                            println!("Successfully sent to OMonitor (default)");
                        }
                    }
                }
            });
        }
    }

    async fn handle_commands(mut command_receiver: crate::channels::CommandReceiver) {
        while let Some(command) = command_receiver.recv().await {
            println!("Received command to write: {} bytes", command.len());
            // Здесь будет логика отправки команд через PWriter
        }
    }

    pub async fn wait_for_completion(&mut self) {
        // Ждем завершения всех задач
        for task in std::mem::take(&mut self.tasks) {
            let _ = task.await;
        }
    }

    pub async fn print_statistics(&self) {
        let sorter_guard = self.p_sorter.lock().await;
        println!("================================================");
        println!("FINAL STATISTICS:");
        println!("Total packages processed: {}", sorter_guard.input_package_counter());
        println!("CRC correct: {}", sorter_guard.crc_correct_counter());
        println!("CRC incorrect: {}", sorter_guard.crc_incorrect_counter());
        println!("Packages sent to TMonitor: {}", sorter_guard.send_pack_forTmon_counter());
        println!("================================================");
    }

}

impl Drop for Controller {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}