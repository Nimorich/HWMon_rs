pub fn from_linear11_f(data: u16) -> f32 {
    if data == 0 {
        return 0.0;
    }
    
    let exponent = ((data >> 11) & 0x001F) as i16;
    let mut mantissa = (data & 0x07FF) as i16;
    
    // Sign extend exponent
    let exponent = if exponent > 0x0F {
        exponent | (0xFFE0u16 as i16)
    } else {
        exponent
    };
    
    // Sign extend mantissa
    if mantissa > 0x03FF {
        mantissa |= 0xF800u16 as i16;
    }
    
    if exponent >= 0 {
        (mantissa as f32) * ((1 << exponent) as f32)
    } else {
        (mantissa as f32) / ((1 << -exponent) as f32)
    }
}