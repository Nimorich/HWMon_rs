use tokio::sync::mpsc;

// Каналы для связи между компонентами
pub type PackageSender = mpsc::UnboundedSender<Vec<u8>>;
pub type PackageReceiver = mpsc::UnboundedReceiver<Vec<u8>>;

pub type CommandSender = mpsc::UnboundedSender<Vec<u8>>;
pub type CommandReceiver = mpsc::UnboundedReceiver<Vec<u8>>;

pub fn package_channel() -> (PackageSender, PackageReceiver) {
    mpsc::unbounded_channel()
}

pub fn command_channel() -> (CommandSender, CommandReceiver) {
    mpsc::unbounded_channel()
}