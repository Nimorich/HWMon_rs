use anyhow::Result;
use zmq;
use std::marker;

pub struct ZmqSender {
    socket: zmq::Socket,
    endpoint: String,
    connected: bool,
    _marker: marker::PhantomData<*const ()>,
}

impl ZmqSender {
    pub fn new(endpoint: &str) -> Self {
        let context = zmq::Context::new();
        let socket = context.socket(zmq::PUB)
            .expect("Failed to create ZeroMQ socket");

        // Configure socket
        socket.set_sndhwm(1000).expect("Failed to set SNDHWM");
        socket.set_linger(0).expect("Failed to set linger");

        let connected = match socket.bind(endpoint) {
            Ok(()) => {
                println!("ZeroMQ PUB socket bound to: {}", endpoint);
                // Small delay for connection stabilization
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

    pub fn is_connected(&self) -> bool {
        self.connected
    }

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
                
                // Try to reconnect on certain errors
                if e.to_string().contains("ETERM") || e.to_string().contains("ENOTSOCK") {
                    eprintln!("ZeroMQ socket invalid, need to recreate");
                }
                Err(anyhow::anyhow!("ZeroMQ send error: {}", e))
            }
        }
    }

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