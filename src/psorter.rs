use crate::include::{byte_stuffing, crc, linear11};

pub type PackageCallback = dyn Fn(i32, &[u8]) + Send + Sync;

#[derive(Debug)]
pub struct PackageStruct {
    pub addr: u16,
    pub module_addr: u8,
    pub module_addr_mcu: u8,
    pub module_addr_bm: u8,
    pub module_id: u8,
    pub package_type: u16,
    pub src: u16,
    pub dev_id: u8,
    pub pwr_line: u8,
    pub src_id: u8,
    pub rtr: bool,
    pub data_type: u16,
    pub prm_id: u16,
    pub alarms: u8,
    pub prm_type: u8,
    pub prm: u16,
    pub prm_max: u16,
    pub prm_min: u16,
    pub temperature: f32,
    pub temp_max: f32,
    pub temp_min: f32,
}

pub struct PSorter {
    input_package_counter: u32,
    crc_correct_counter: u32,
    crc_incorrect_counter: u32,
    send_pack_forTmon_counter: u32,
}

impl PSorter {
    pub fn new() -> Self {
        Self {
            input_package_counter: 0,
            crc_correct_counter: 0,
            crc_incorrect_counter: 0,
            send_pack_forTmon_counter: 0,
        }
    }
    pub fn input_package_counter(&self) -> u32 {self.input_package_counter}
    pub fn crc_correct_counter(&self) -> u32 {self.crc_correct_counter}
    pub fn crc_incorrect_counter(&self) -> u32 {self.crc_incorrect_counter}
    pub fn send_pack_forTmon_counter(&self) -> u32 {self.send_pack_forTmon_counter}
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

    let pack_stuffed = byte_stuffing::byte_stuffing(package);

    let is_crc = self.crc_correct(&pack_stuffed);
    if is_crc {
        self.crc_correct_counter += 1;
        let pack_struct = self.make_package_struct(&pack_stuffed);
        
        // Отладочная информация о структуре пакета
        println!("Parsed package - BM:{}, FPGA:{}, Type:0x{:04x}, PRM_ID:{}, PRM_TYPE:{}", 
                 pack_struct.module_addr_bm, pack_struct.dev_id, 
                 pack_struct.package_type, pack_struct.prm_id, pack_struct.prm_type);
        
        let pack_type = self.package_identificator(&pack_struct);
        
        // Вызываем колбэк с типом пакета и данными
        callback(pack_type, &pack_stuffed);
        
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

        if input_package.len() < 14 {
            return ps;
        }

        let mask_1b = 0x0001;
        let mask_2b = 0x0003;
        let mask_3b = 0x0007;
        let mask_4b = 0x000F;
        let mask_5b = 0x001F;
        let mask_7b = 0x007F;
        let mask_10b = 0x03FF;

        // Extract addr (bytes 0-1) - little endian
        ps.addr = ((input_package[1] as u16) << 8) | (input_package[0] as u16);
        ps.module_addr = (ps.addr & mask_7b) as u8;
        ps.module_addr_mcu = (ps.module_addr as u16 & mask_3b) as u8;
        ps.module_addr_bm = (ps.module_addr >> 3) as u8;
        ps.module_id = ((ps.addr >> 7) & mask_4b) as u8;

        // Extract package_type (bytes 2-3)
        ps.package_type = ((input_package[3] as u16) << 8) | (input_package[2] as u16);

        // Extract src (bytes 4-5)
        ps.src = ((input_package[5] as u16) << 8) | (input_package[4] as u16);
        ps.dev_id = (ps.src & mask_7b) as u8;
        ps.pwr_line = ((ps.src >> 7) & mask_4b) as u8;
        ps.src_id = ((ps.src >> 11) & mask_5b) as u8;
        ps.rtr = ((ps.src >> 15) & mask_1b) != 0;

        // Extract data_type (bytes 6-7)
        ps.data_type = ((input_package[7] as u16) << 8) | (input_package[6] as u16);
        ps.prm_id = ps.data_type & mask_10b;
        ps.alarms = ((ps.data_type >> 10) & mask_4b) as u8;
        ps.prm_type = ((ps.data_type >> 14) & mask_2b) as u8;

        // Extract prm (bytes 8-9)
        ps.prm = ((input_package[9] as u16) << 8) | (input_package[8] as u16);

        // Extract prm_max (bytes 10-11)
        ps.prm_max = ((input_package[11] as u16) << 8) | (input_package[10] as u16);

        // Extract prm_min (bytes 12-13)
        ps.prm_min = ((input_package[13] as u16) << 8) | (input_package[12] as u16);

        // Convert from Linear11 to float
        ps.temperature = linear11::from_linear11_f(ps.prm);
        ps.temp_max = linear11::from_linear11_f(ps.prm_max);
        ps.temp_min = linear11::from_linear11_f(ps.prm_min);

        ps
    }

    fn package_identificator(&self, ps: &PackageStruct) -> i32 {
        let min_bm = 0;
        let max_bm = 15;

        // Temperature packages
        if ps.module_addr_mcu == 1 || ps.module_addr_mcu == 2 {
            if ps.module_id == 2 { // BM module
                if ps.module_addr_bm >= min_bm && ps.module_addr_bm <= max_bm {
                    if ps.package_type == 32768 { // 0x8000
                        if ps.prm_type == 0 || ps.prm_type == 1 || ps.prm_type == 2 { // Parameter value (not min/max)
                            if ps.src_id == 2 || ps.src_id == 3 { // FPGA
                                if ps.prm_id == 10 || ps.prm_id == 11 || ps.prm_id == 12 {
                                    if ps.dev_id >= 1 && ps.dev_id <= 6 {
                                        println!("Identified as TEMPERATURE package: BM{}, FPGA{}, PRM_ID: {}", 
                                                 ps.module_addr_bm, ps.dev_id, ps.prm_id);
                                        return 1; // Temperature data
                                    } else {
                                        return 5; // Control data
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // System packages (добавьте условия для системных пакетов)
        if ps.package_type == 0x8000 && ps.prm_id == 20 {
            println!("Identified as SYSTEM package");
            return 2;
        }

        // Power usage packages (добавьте условия для пакетов энергопотребления)
        if ps.package_type == 0x8000 && ps.prm_id == 30 {
            println!("Identified as POWER USAGE package");
            return 3;
        }

        // Control packages
        if ps.dev_id > 6 || ps.module_addr_bm > max_bm {
            println!("Identified as CONTROL package");
            return 5;
        }

        // Default to Overview
        println!("Identified as OVERVIEW package - BM:{}, FPGA:{}, Type:{}, PRM_ID:{}", 
                 ps.module_addr_bm, ps.dev_id, ps.package_type, ps.prm_id);
        4
    }

    fn crc_correct(&self, byte_buffer: &[u8]) -> bool {
        if byte_buffer.len() < 2 {
            return false;
        }
        
        let crc_received = ((byte_buffer[byte_buffer.len() - 2] as u16) << 8) 
            | (byte_buffer[byte_buffer.len() - 1] as u16);

        crc::crc16_validate(&byte_buffer[0..byte_buffer.len() - 2], crc_received)
    }
}