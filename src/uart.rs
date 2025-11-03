use anyhow::{Context, Result};
use serialport::SerialPort;
use std::time::Duration;

pub struct Uart {
    port: Box<dyn SerialPort>,
}

impl Uart {
    pub fn new() -> Result<Self> {
        let port = serialport::new("/dev/ttyS0", 1_000_000)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .timeout(Duration::from_millis(1000))
            .open()
            .context("Failed to open serial port")?;

        println!("UART port opened successfully");
        Ok(Uart { port })
    }

    pub fn is_open(&self) -> bool {
        true // serialport всегда открыт если создан успешно
    }

    pub fn is_readable(&self) -> bool {
        true // Предполагаем что порт всегда читаем
    }

    pub fn read_all(&mut self) -> Result<Vec<u8>> {
        let mut buffer = vec![0; 1024];
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

    pub fn write_all(&mut self, data: &[u8]) -> Result<()> {
        self.port.write_all(data)
            .context("Failed to write to serial port")?;
        
        self.port.flush()
            .context("Failed to flush serial port")?;
        
        println!("UART wrote {} bytes", data.len());
        Ok(())
    }
}

impl Drop for Uart {
    fn drop(&mut self) {
        println!("UART port closed");
    }
}