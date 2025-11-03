use anyhow::Result;
use zmq;
use std::marker;

/// Структура для отправки данных через ZeroMQ PUB socket
/// Используется для передачи данных различным мониторам (TMonitor, SMonitor и др.)
pub struct ZmqSender {
    socket: zmq::Socket,               // ZeroMQ сокет
    endpoint: String,                  // Адрес конечной точки
    connected: bool,                   // Флаг подключения
    _marker: marker::PhantomData<*const ()>,  // Маркер для безопасности памяти
}

impl ZmqSender {
    /// Создает новый ZeroMQ отправитель с указанным адресом
    pub fn new(endpoint: &str) -> Self {
        let context = zmq::Context::new();
        let socket = context.socket(zmq::PUB)
            .expect("Failed to create ZeroMQ socket");

        // Настройка параметров сокета
        socket.set_sndhwm(1000).expect("Failed to set SNDHWM");  // High watermark для отправки
        socket.set_linger(0).expect("Failed to set linger");     // Немедленное закрытие

        // Попытка привязать сокет к адресу
        let connected = match socket.bind(endpoint) {
            Ok(()) => {
                println!("ZeroMQ PUB socket bound to: {}", endpoint);
                // Небольшая задержка для стабилизации соединения
                std::thread::sleep(std::time::Duration::from_millis(100));
                true
            }
            Err(e) => {
                eprintln!("ZeroMQ bind error: {}", e);
                false
            }
        };

        Self {
            socket,
            endpoint: endpoint.to_string(),
            connected,
            _marker: marker::PhantomData,
        }
    }

    /// Проверяет, подключен ли сокет
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Отправляет данные через ZeroMQ сокет
    pub fn send_data(&self, data: &[u8]) -> Result<()> {
        if !self.connected || data.is_empty() {
            return Ok(());
        }

        match self.socket.send(data, 0) {
            Ok(()) => {
                println!("ZeroMQ: Data sent, size: {}", data.len());
                Ok(())
            }
            Err(e) => {
                eprintln!("ZeroMQ send error: {}", e);
                
                // Попытка переподключения при определенных ошибках
                if e.to_string().contains("ETERM") || e.to_string().contains("ENOTSOCK") {
                    eprintln!("ZeroMQ socket invalid, need to recreate");
                }
                Err(anyhow::anyhow!("ZeroMQ send error: {}", e))
            }
        }
    }

    /// Отправляет пакет данных с дополнительным логированием
    pub fn send_package(&self, pkg: &[u8]) -> Result<()> {
    println!("ZMQ: Attempting to send package, size: {}", pkg.len());
    let result = self.send_data(pkg);
    match &result {
        Ok(_) => println!("ZMQ: Package sent successfully"),
        Err(e) => eprintln!("ZMQ: Failed to send package: {}", e),
    }
        result
    }
}

/// Реализация деструктора для корректного закрытия сокета
impl Drop for ZmqSender {
    fn drop(&mut self) {
        if self.connected {
            self.socket.disconnect(&self.endpoint).ok();
        }
    }
}

// Безопасно делаем ZmqSender Send, так как мы используем его правильно
unsafe impl Send for ZmqSender {}
unsafe impl Sync for ZmqSender {}