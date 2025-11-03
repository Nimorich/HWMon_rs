/// Функции для байт-стаффинга (byte stuffing) - алгоритма экранирования специальных байтов
/// Используется для передачи данных с байтом-разделителем 0xC0

/// Восстанавливает исходные данные из стаффированного буфера
/// Заменяет escape-последовательности на исходные байты:
/// - \xDB\xDC → \xC0 (разделитель пакетов)
/// - \xDB\xDD → \xDB (экранированный байт 0xDB)
pub fn byte_stuffing(byte_buffer: &[u8]) -> Vec<u8> {
    let mut buffer_stuff = Vec::new();
    let mut i = 0;

    // Проходим по всем байтам буфера
    while i < byte_buffer.len() {
        if byte_buffer[i] == 0xDB {
            // Найден байт-экранировщик, проверяем следующий байт
            if i + 1 < byte_buffer.len() {
                match byte_buffer[i + 1] {
                    0xDC => {
                        buffer_stuff.push(0xC0); // Заменяем \xDB\xDC на \xC0
                        i += 2; // Пропускаем следующий байт
                    }
                    0xDD => {
                        buffer_stuff.push(0xDB); // Заменяем \xDB\xDD на \xDB
                        i += 2; // Пропускаем следующий байт
                    }
                    _ => {
                        buffer_stuff.push(0xDB); // Нормальный байт 0xDB
                        i += 1;
                    }
                }
            } else {
                buffer_stuff.push(0xDB); // Конец массива
                i += 1;
            }
        } else {
            buffer_stuff.push(byte_buffer[i]); // Добавляем обычный байт
            i += 1;
        }
    }

    buffer_stuff
}

/// Применяет байт-стаффинг к команде перед отправкой
/// Экранирует специальные байты escape-последовательностями:
/// - \xC0 → \xDB\xDC (разделитель пакетов)
/// - \xDB → \xDB\xDD (экранированный байт 0xDB)
pub fn request_byte_stuffing(command_request: &mut Vec<u8>) {
    let mut i = 0;
    while i < command_request.len() {
        match command_request[i] {
            0xC0 => {
                // Экранируем байт-разделитель
                command_request[i] = 0xDB;
                command_request.insert(i + 1, 0xDC);
                i += 2;
            }
            0xDB => {
                // Экранируем байт-экранировщик
                command_request.insert(i + 1, 0xDD);
                i += 2;
            }
            _ => {
                // Обычный байт, пропускаем
                i += 1;
            }
        }
    }
}