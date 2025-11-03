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

/// Тип операции чтения данных
#[derive(Debug, Clone, PartialEq)]
pub enum ReadOperation {
    /// Чтение данных через UART (последовательный порт)
    Uart,
    /// Чтение данных из дампа файла
    Dump,
    /// Чтение данных через CAN-шину (не реализовано)
    Can,
}

/// Основной контроллер приложения, управляющий всеми компонентами
pub struct Controller {
    // ZMQ отправители для различных типов мониторов
    sender_t: Arc<ZmqSender>,  // Для TMonitor
    sender_s: Arc<ZmqSender>,  // Для SMonitor
    sender_pu: Arc<ZmqSender>, // Для PUMonitor
    sender_o: Arc<ZmqSender>,  // Для OMonitor
    sender_c: Arc<ZmqSender>,  // Для CMonitor
    
    // Каналы для передачи данных между компонентами
    package_sender: PackageSender,  // Для отправки пакетов на сортировку
    command_sender: CommandSender,  // Для отправки команд на запись
    
    // Компоненты системы
    dump_reader: Option<DumpReader>,          // Читатель дамп-файлов
    uart: Option<Arc<Mutex<Uart>>>,           // UART интерфейс (защищен мьютексом)
    p_reader: Option<Arc<Mutex<PReader>>>,    // Читатель пакетов
    p_writer: Option<Arc<Mutex<PWriter>>>,    // Писатель пакетов (не используется)
    p_sorter: Arc<Mutex<PSorter>>,            // Сортировщик пакетов
    
    // Состояние контроллера
    dump_filename: Option<String>,    // Имя файла дампа (если используется режим Dump)
    read_operation: ReadOperation,    // Текущий режим чтения
    
    // Асинхронные задачи, выполняемые контроллером
    tasks: Vec<JoinHandle<()>>,
}

impl Controller {
    /// Создает новый контроллер и запускает фоновые задачи
    pub async fn new() -> Result<Self> {
        // Инициализация ZMQ отправителей на разных портах
        let sender_t = Arc::new(ZmqSender::new("tcp://*:5555"));  // TMonitor
        let sender_s = Arc::new(ZmqSender::new("tcp://*:5556"));  // SMonitor
        let sender_pu = Arc::new(ZmqSender::new("tcp://*:5557")); // PUMonitor
        let sender_o = Arc::new(ZmqSender::new("tcp://*:5558"));  // OMonitor
        let sender_c = Arc::new(ZmqSender::new("tcp://*:5559"));  // CMonitor

        // Создание каналов для межкомпонентного взаимодействия
        let (package_sender, package_receiver) = package_channel();   // Канал для пакетов
        let (command_sender, command_receiver) = command_channel();   // Канал для команд

        // Инициализация сортировщика пакетов
        let sorter = Arc::new(Mutex::new(PSorter::new()));
        let sorter_clone = Arc::clone(&sorter);
        
        // Подготовка клонов для использования в асинхронной задаче
        let sender_t_clone = Arc::clone(&sender_t);
        let sender_s_clone = Arc::clone(&sender_s);
        let sender_pu_clone = Arc::clone(&sender_pu);
        let sender_o_clone = Arc::clone(&sender_o);
        let sender_c_clone = Arc::clone(&sender_c);

        // Запуск задачи обработки пакетов
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

        // Запуск задачи обработки команд
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
            read_operation: ReadOperation::Uart,  // По умолчанию UART режим
            tasks: vec![package_handler, command_handler],
        })
    }

    /// Устанавливает режим чтения данных
    pub fn set_read_operation(&mut self, operation: ReadOperation) {
        self.read_operation = operation.clone();
        println!("Read operation changed to: {:?}", operation);
    }

    /// Устанавливает имя файла дампа для режима Dump
    pub fn set_dump_filename(&mut self, filename: String) {
        self.dump_filename = Some(filename);
    }

    /// Запускает контроллер в выбранном режиме чтения
    pub async fn start(&mut self) -> Result<()> {
        println!("================================================");
        println!("Starting server with ZeroMQ endpoints:");
        println!("TMonitor:  tcp://localhost:5555");   // Порт для TMonitor
        println!("SMonitor:  tcp://localhost:5556");   // Порт для SMonitor
        println!("PUMonitor: tcp://localhost:5557");   // Порт для PUMonitor
        println!("OMonitor:  tcp://localhost:5558");   // Порт для OMonitor
        println!("CMonitor:  tcp://localhost:5559");   // Порт для CMonitor
        println!("Current read operation: {:?}", self.read_operation);
        println!("================================================");

        // Запуск в зависимости от выбранного режима
        match self.read_operation {
            ReadOperation::Uart => {
                self.start_uart_mode().await?;  // Режим чтения через UART
            }
            ReadOperation::Dump => {
                self.start_dump_mode().await?;  // Режим чтения из файла дампа
            }
            ReadOperation::Can => {
                println!("CAN mode selected - not implemented yet");  // CAN режим не реализован
            }
        }

        Ok(())
    }

    /// Запускает режим чтения через UART
    async fn start_uart_mode(&mut self) -> Result<()> {
        // Создание UART интерфейса
        let uart = Arc::new(Mutex::new(Uart::new()?));
        
        // Создание читателя пакетов
        let mut p_reader = PReader::new(
            Arc::clone(&uart),
            self.package_sender.clone(),
        );
        
        // Запуск задачи чтения из UART
        let uart_task = tokio::spawn(async move {
            // Начало чтения (синхронная операция)
            if let Err(e) = p_reader.start_reading() {
                eprintln!("Failed to start UART reading: {}", e);
                return;
            }
            
            // Основной цикл чтения (асинхронная операция)
            if let Err(e) = p_reader.read_loop().await {
                eprintln!("UART reading error: {}", e);
            }
        });
        
        // Сохранение ссылок на компоненты
        self.uart = Some(uart);
        self.tasks.push(uart_task);
        
        println!("UART mode started - reading continuously");
        Ok(())
    }

    /// Запускает режим чтения из файла дампа
    async fn start_dump_mode(&mut self) -> Result<()> {
        // Проверка наличия имени файла дампа
        let filename = self.dump_filename.as_ref()
            .context("Dump filename not set")?;
        
        // Создание читателя дампа
        let dump_reader = DumpReader::new(self.package_sender.clone()); // убираем mut
        let filename_clone = filename.clone();
        
        // Запуск задачи чтения дампа
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

    /// Обрабатывает входящие пакеты и распределяет их по соответствующим ZMQ отправителям
    async fn handle_packages(
        mut package_receiver: crate::channels::PackageReceiver,  // Приемник пакетов
        sorter: Arc<Mutex<PSorter>>,                             // Сортировщик пакетов
        sender_t: Arc<ZmqSender>,                                // Отправитель для TMonitor
        sender_s: Arc<ZmqSender>,                                // Отправитель для SMonitor
        sender_pu: Arc<ZmqSender>,                               // Отправитель для PUMonitor
        sender_o: Arc<ZmqSender>,                                // Отправитель для OMonitor
        sender_c: Arc<ZmqSender>,                                // Отправитель для CMonitor
    ) {
        // Основной цикл обработки пакетов
        while let Some(package) = package_receiver.recv().await {
            // Блокировка сортировщика для обработки пакета
            let mut sorter_guard = sorter.lock().await;
            
            // Создание клонов отправителей для использования в замыкании
            let sender_t = Arc::clone(&sender_t);
            let sender_s = Arc::clone(&sender_s);
            let sender_pu = Arc::clone(&sender_pu);
            let sender_o = Arc::clone(&sender_o);
            let sender_c = Arc::clone(&sender_c);
            
            // Обработка пакета через сортировщик
            sorter_guard.slot_input_package(&package, move |pack_type, data| {
                // Распределение пакета по типам
                match pack_type {
                    1 => {  // Пакет для TMonitor
                        if let Err(e) = sender_t.send_package(data) {
                            eprintln!("Failed to send to TMonitor: {}", e);
                        } else {
                            println!("Successfully sent to TMonitor");
                        }
                    }
                    2 => {  // Пакет для SMonitor
                        if let Err(e) = sender_s.send_package(data) {
                            eprintln!("Failed to send to SMonitor: {}", e);
                        } else {
                            println!("Successfully sent to SMonitor");
                        }
                    }
                    3 => {  // Пакет для PUMonitor
                        if let Err(e) = sender_pu.send_package(data) {
                            eprintln!("Failed to send to PUMonitor: {}", e);
                        } else {
                            println!("Successfully sent to PUMonitor");
                        }
                    }
                    4 => {  // Пакет для OMonitor
                        if let Err(e) = sender_o.send_package(data) {
                            eprintln!("Failed to send to OMonitor: {}", e);
                        } else {
                            println!("Successfully sent to OMonitor");
                        }
                    }
                    5 => {  // Пакет для CMonitor
                        if let Err(e) = sender_c.send_package(data) {
                            eprintln!("Failed to send to CMonitor: {}", e);
                        } else {
                            println!("Successfully sent to CMonitor");
                        }
                    }
                    6 => {  // Неиспользуемый тип пакета
                        // Пустая обработка для типа 6
                    }
                    _ => {  // Пакет неизвестного типа - отправляем в OMonitor по умолчанию
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

    /// Обрабатывает входящие команды (в настоящее время только логирует)
    async fn handle_commands(mut command_receiver: crate::channels::CommandReceiver) {
        while let Some(command) = command_receiver.recv().await {
            println!("Received command to write: {} bytes", command.len());
            // Здесь будет логика отправки команд через PWriter
            // В текущей реализации команды только логируются
        }
    }

    /// Ожидает завершения всех асинхронных задач
    pub async fn wait_for_completion(&mut self) {
        // Ждем завершения всех задач
        for task in std::mem::take(&mut self.tasks) {
            let _ = task.await;  // Игнорируем результат, так как задачи возвращают ()
        }
    }

    /// Выводит финальную статистику работы системы
    pub async fn print_statistics(&self) {
        let sorter_guard = self.p_sorter.lock().await;
        println!("================================================");
        println!("FINAL STATISTICS:");
        println!("Total packages processed: {}", sorter_guard.input_package_counter());      // Всего обработано пакетов
        println!("CRC correct: {}", sorter_guard.crc_correct_counter());                     // Пакетов с корректным CRC
        println!("CRC incorrect: {}", sorter_guard.crc_incorrect_counter());                 // Пакетов с некорректным CRC
        println!("Packages sent to TMonitor: {}", sorter_guard.send_pack_forTmon_counter()); // Пакетов отправлено в TMonitor
        println!("================================================");
    }
}

/// Реализация деструктора для корректного завершения асинхронных задач
impl Drop for Controller {
    fn drop(&mut self) {
        // При уничтожении контроллера прерываем все асинхронные задачи
        for task in &self.tasks {
            task.abort();
        }
    }
}