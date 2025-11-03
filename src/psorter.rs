use crate::include::{byte_stuffing, crc, linear11};

/// Тип колбэк-функции для обработки отсортированных пакетов
pub type PackageCallback = dyn Fn(i32, &[u8]) + Send + Sync;

/// Структура для хранения разобранных данных пакета
#[derive(Debug)]
pub struct PackageStruct {
    pub addr: u16,                    // Адрес модуля
    pub module_addr: u8,              // Адрес модуля (7 бит)
    pub module_addr_mcu: u8,          // Адрес MCU (3 бита)
    pub module_addr_bm: u8,           // Адрес BM (4 бита)
    pub module_id: u8,                // Идентификатор модуля (4 бита)
    pub package_type: u16,            // Тип пакета
    pub src: u16,                     // Источник данных
    pub dev_id: u8,                   // Идентификатор устройства (7 бит)
    pub pwr_line: u8,                 // Линия питания (4 бита)
    pub src_id: u8,                   // Идентификатор источника (5 бит)
    pub rtr: bool,                    // Флаг RTR (Remote Transmission Request)
    pub data_type: u16,               // Тип данных
    pub prm_id: u16,                  // Идентификатор параметра (10 бит)
    pub alarms: u8,                   // Аварийные сигналы (4 бита)
    pub prm_type: u8,                 // Тип параметра (2 бита)
    pub prm: u16,                     // Значение параметра
    pub prm_max: u16,                 // Максимальное значение параметра
    pub prm_min: u16,                 // Минимальное значение параметра
    pub temperature: f32,             // Температура (преобразованная из Linear11)
    pub temp_max: f32,                // Максимальная температура
    pub temp_min: f32,                // Минимальная температура
}

/// Сортировщик пакетов - анализирует входящие пакеты и распределяет их по типам
pub struct PSorter {
    input_package_counter: u32,        // Счетчик принятых пакетов
    crc_correct_counter: u32,          // Счетчик пакетов с корректным CRC
    crc_incorrect_counter: u32,        // Счетчик пакетов с некорректным CRC
    send_pack_forTmon_counter: u32,    // Счетчик пакетов для TMonitor
}

impl PSorter {
    /// Создает новый сортировщик пакетов
    pub fn new() -> Self {
        Self {
            input_package_counter: 0,
            crc_correct_counter: 0,
            crc_incorrect_counter: 0,
            send_pack_forTmon_counter: 0,
        }
    }
    
    /// Возвращает количество принятых пакетов
    pub fn input_package_counter(&self) -> u32 {self.input_package_counter}
    
    /// Возвращает количество пакетов с корректным CRC
    pub fn crc_correct_counter(&self) -> u32 {self.crc_correct_counter}
    
    /// Возвращает количество пакетов с некорректным CRC
    pub fn crc_incorrect_counter(&self) -> u32 {self.crc_incorrect_counter}
    
    /// Возвращает количество пакетов отправленных в TMonitor
    pub fn send_pack_forTmon_counter(&self) -> u32 {self.send_pack_forTmon_counter}
    
    /// Основной метод обработки входящего пакета
    /// Принимает пакет и колбэк для отправки отсортированного пакета
    pub fn slot_input_package<F>(&mut self, package: &[u8], callback: F) 
where
    F: Fn(i32, &[u8]) + Send + Sync,
{
    if package.is_empty() {
        println!("PSorter: Empty package received");
        return;
    }

    self.input_package_counter += 1;
    println!("pSorter принял пакет: {}", hex::encode(package));

    // Применяем байт-стаффинг для восстановления исходных данных
    let pack_stuffed = byte_stuffing::byte_stuffing(package);

    // Проверяем корректность CRC
    let is_crc = self.crc_correct(&pack_stuffed);
    if is_crc {
        self.crc_correct_counter += 1;
        
        // Разбираем пакет в структуру
        let pack_struct = self.make_package_struct(&pack_stuffed);
        
        // Отладочная информация о структуре пакета
        println!("Parsed package - BM:{}, FPGA:{}, Type:0x{:04x}, PRM_ID:{}, PRM_TYPE:{}", 
                 pack_struct.module_addr_bm, pack_struct.dev_id, 
                 pack_struct.package_type, pack_struct.prm_id, pack_struct.prm_type);
        
        // Определяем тип пакета
        let pack_type = self.package_identificator(&pack_struct);
        
        // Вызываем колбэк с типом пакета и данными
        callback(pack_type, &pack_stuffed);
        
        // Логируем назначение пакета
        match pack_type {
            1 => {println!(">>> to TMonitor"); self.send_pack_forTmon_counter += 1},
            2 => println!(">>> to SMonitor"),
            3 => println!(">>> to PUMonitor"),
            4 => println!(">>> to OMonitor"),
            5 => println!(">>> to CMonitor"),
            _ => println!(">>> to Overview"),
        }
    } else {
        self.crc_incorrect_counter += 1;
        println!("CRC incorrect for package");
    }
    }

    /// Разбирает байтовый буфер в структурированные данные пакета
    fn make_package_struct(&self, input_package: &[u8]) -> PackageStruct {
        let mut ps = PackageStruct {
            addr: 0,
            module_addr: 0,
            module_addr_mcu: 0,
            module_addr_bm: 0,
            module_id: 0,
            package_type: 0,
            src: 0,
            dev_id: 0,
            pwr_line: 0,
            src_id: 0,
            rtr: false,
            data_type: 0,
            prm_id: 0,
            alarms: 0,
            prm_type: 0,
            prm: 0,
            prm_max: 0,
            prm_min: 0,
            temperature: 0.0,
            temp_max: 0.0,
            temp_min: 0.0,
        };

        // Проверяем минимальную длину пакета
        if input_package.len() < 14 {
            return ps;
        }

        // Маски для извлечения битовых полей
        let mask_1b = 0x0001;
        let mask_2b = 0x0003;
        let mask_3b = 0x0007;
        let mask_4b = 0x000F;
        let mask_5b = 0x001F;
        let mask_7b = 0x007F;
        let mask_10b = 0x03FF;

        // Извлекаем адрес (байты 0-1) - little endian
        ps.addr = ((input_package[1] as u16) << 8) | (input_package[0] as u16);
        ps.module_addr = (ps.addr & mask_7b) as u8;
        ps.module_addr_mcu = (ps.module_addr as u16 & mask_3b) as u8;
        ps.module_addr_bm = (ps.module_addr >> 3) as u8;
        ps.module_id = ((ps.addr >> 7) & mask_4b) as u8;

        // Извлекаем тип пакета (байты 2-3)
        ps.package_type = ((input_package[3] as u16) << 8) | (input_package[2] as u16);

        // Извлекаем источник данных (байты 4-5)
        ps.src = ((input_package[5] as u16) << 8) | (input_package[4] as u16);
        ps.dev_id = (ps.src & mask_7b) as u8;
        ps.pwr_line = ((ps.src >> 7) & mask_4b) as u8;
        ps.src_id = ((ps.src >> 11) & mask_5b) as u8;
        ps.rtr = ((ps.src >> 15) & mask_1b) != 0;

        // Извлекаем тип данных (байты 6-7)
        ps.data_type = ((input_package[7] as u16) << 8) | (input_package[6] as u16);
        ps.prm_id = ps.data_type & mask_10b;
        ps.alarms = ((ps.data_type >> 10) & mask_4b) as u8;
        ps.prm_type = ((ps.data_type >> 14) & mask_2b) as u8;

        // Извлекаем значение параметра (байты 8-9)
        ps.prm = ((input_package[9] as u16) << 8) | (input_package[8] as u16);

        // Извлекаем максимальное значение параметра (байты 10-11)
        ps.prm_max = ((input_package[11] as u16) << 8) | (input_package[10] as u16);

        // Извлекаем минимальное значение параметра (байты 12-13)
        ps.prm_min = ((input_package[13] as u16) << 8) | (input_package[12] as u16);

        // Преобразуем из формата Linear11 в float
        ps.temperature = linear11::from_linear11_f(ps.prm);
        ps.temp_max = linear11::from_linear11_f(ps.prm_max);
        ps.temp_min = linear11::from_linear11_f(ps.prm_min);

        ps
    }

    /// Определяет тип пакета на основе его структуры
    fn package_identificator(&self, ps: &PackageStruct) -> i32 {
        let min_bm = 0;
        let max_bm = 15;

        // Пакеты температуры
        if ps.module_addr_mcu == 1 || ps.module_addr_mcu == 2 {
            if ps.module_id == 2 { // BM module
                if ps.module_addr_bm >= min_bm && ps.module_addr_bm <= max_bm {
                    if ps.package_type == 32768 { // 0x8000
                        if ps.prm_type == 0 || ps.prm_type == 1 || ps.prm_type == 2 { // Значение параметра (не min/max)
                            if ps.src_id == 2 || ps.src_id == 3 { // FPGA
                                if ps.prm_id == 10 || ps.prm_id == 11 || ps.prm_id == 12 {
                                    if ps.dev_id >= 1 && ps.dev_id <= 6 {
                                        println!("Identified as TEMPERATURE package: BM{}, FPGA{}, PRM_ID: {}", 
                                                 ps.module_addr_bm, ps.dev_id, ps.prm_id);
                                        return 1; // Данные температуры
                                    } else {
                                        return 5; // Данные управления
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Системные пакеты
        if ps.package_type == 0x8000 && ps.prm_id == 20 {
            println!("Identified as SYSTEM package");
            return 2;
        }

        // Пакеты энергопотребления
        if ps.package_type == 0x8000 && ps.prm_id == 30 {
            println!("Identified as POWER USAGE package");
            return 3;
        }

        // Пакеты управления
        if ps.dev_id > 6 || ps.module_addr_bm > max_bm {
            println!("Identified as CONTROL package");
            return 5;
        }

        // По умолчанию - обзорные пакеты
        println!("Identified as OVERVIEW package - BM:{}, FPGA:{}, Type:{}, PRM_ID:{}", 
                 ps.module_addr_bm, ps.dev_id, ps.package_type, ps.prm_id);
        4
    }

    /// Проверяет корректность CRC пакета
    fn crc_correct(&self, byte_buffer: &[u8]) -> bool {
        if byte_buffer.len() < 2 {
            return false;
        }
        
        // Извлекаем CRC из пакета (последние 2 байта)
        let crc_received = ((byte_buffer[byte_buffer.len() - 2] as u16) << 8) 
            | (byte_buffer[byte_buffer.len() - 1] as u16);

        // Проверяем CRC для данных без последних 2 байт (самого CRC)
        crc::crc16_validate(&byte_buffer[0..byte_buffer.len() - 2], crc_received)
    }
}