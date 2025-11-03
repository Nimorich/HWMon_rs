use tokio::sync::mpsc;

// Каналы для связи между компонентами системы
// Типы для передачи пакетов данных между компонентами
pub type PackageSender = mpsc::UnboundedSender<Vec<u8>>;     // Отправитель пакетов данных
pub type PackageReceiver = mpsc::UnboundedReceiver<Vec<u8>>;  // Получатель пакетов данных

// Типы для передачи команд управления между компонентами
pub type CommandSender = mpsc::UnboundedSender<Vec<u8>>;      // Отправитель команд
pub type CommandReceiver = mpsc::UnboundedReceiver<Vec<u8>>;  // Получатель команд

/// Создает неограниченный канал для передачи пакетов данных
/// Используется для передачи пакетов от читателей к сортировщику
pub fn package_channel() -> (PackageSender, PackageReceiver) {
    mpsc::unbounded_channel()
}

/// Создает неограниченный канал для передачи команд управления
/// Используется для отправки команд на запись данных
pub fn command_channel() -> (CommandSender, CommandReceiver) {
    mpsc::unbounded_channel()
}