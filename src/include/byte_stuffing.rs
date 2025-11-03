pub fn byte_stuffing(byte_buffer: &[u8]) -> Vec<u8> {
    let mut buffer_stuff = Vec::new();
    let mut i = 0;

    while i < byte_buffer.len() {
        if byte_buffer[i] == 0xDB {
            if i + 1 < byte_buffer.len() {
                match byte_buffer[i + 1] {
                    0xDC => {
                        buffer_stuff.push(0xC0); // Replace \xDB\xDC with \xC0
                        i += 2; // Skip next byte
                    }
                    0xDD => {
                        buffer_stuff.push(0xDB); // Replace \xDB\xDD with \xDB
                        i += 2; // Skip next byte
                    }
                    _ => {
                        buffer_stuff.push(0xDB); // Normal case
                        i += 1;
                    }
                }
            } else {
                buffer_stuff.push(0xDB); // End of array
                i += 1;
            }
        } else {
            buffer_stuff.push(byte_buffer[i]); // Add normal byte
            i += 1;
        }
    }

    buffer_stuff
}

pub fn request_byte_stuffing(command_request: &mut Vec<u8>) {
    let mut i = 0;
    while i < command_request.len() {
        match command_request[i] {
            0xC0 => {
                command_request[i] = 0xDB;
                command_request.insert(i + 1, 0xDC);
                i += 2;
            }
            0xDB => {
                command_request.insert(i + 1, 0xDD);
                i += 2;
            }
            _ => {
                i += 1;
            }
        }
    }
}