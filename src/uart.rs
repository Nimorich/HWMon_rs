use anyhow::{Context, Result};
use serialport::SerialPort;
use std::time::Duration;

/// Структура для работы с последовательным портом (UART)
pub struct Uart {
    port: Box<dyn SerialPort>,  // Объект последовательного порта
}

impl Uart {
    /// Создает новый экземпляр UART с настройками по умолчанию
    /// Настройки: 1 Мбит/с, 8 бит данных, без контроля четности, 1 стоп-бит
    pub fn new() -> Result<Self> {
        let port = serialport::new("/dev/ttyS0", 1_000_000)
            .data_bits(serialport::DataBits::Eight)     // 8 бит данных
            .parity(serialport::Parity::None)           // Без контроля четности
            .stop_bits(serialport::StopBits::One)       // 1 стоп-бит
            .flow_control(serialport::FlowControl::None) // Без управления потоком
            .timeout(Duration::from_millis(1000))       // Таймаут 1 секунда
            .open()
            .context("Failed to open serial port")?;

        println!("UART port opened successfully");
        Ok(Uart { port })
    }

    /// Проверяет, открыт ли порт
    /// В данной реализации всегда возвращает true, так как порт открывается при создании
    pub fn is_open(&self) -> bool {
        true // serialport всегда открыт если создан успешно
    }

    /// Проверяет, доступен ли порт для чтения
    /// В данной реализации всегда возвращает true
    pub fn is_readable(&self) -> bool {
        true // Предполагаем что порт всегда читаем
    }

    /// Читает все доступные данные из порта
    /// Возвращает вектор байтов или пустой вектор при таймауте
    pub fn read_all(&mut self) -> Result<Vec<u8>> {
        let mut buffer = vec![0; 1024];  // Буфер размером 1 КБ
        match self.port.read(&mut buffer) {
            Ok(size) => {
                buffer.truncate(size);
                if size > 0 {
                    println!("UART read {} bytes", size);
                }
                Ok(buffer)
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                Ok(Vec::new()) // Таймаут, нет данных
            }
            Err(e) => Err(anyhow::anyhow!("Serial port read error: {}", e)),
        }
    }

    /// Записывает данные в порт
    pub fn write_all(&mut self, data: &[u8]) -> Result<()> {
        self.port.write_all(data)
            .context("Failed to write to serial port")?;
        
        // Обеспечиваем отправку всех данных
        self.port.flush()
            .context("Failed to flush serial port")?;
        
        println!("UART wrote {} bytes", data.len());
        Ok(())
    }
}

/// Реализация деструктора для корректного закрытия порта
impl Drop for Uart {
    fn drop(&mut self) {
        println!("UART port closed");
    }
}