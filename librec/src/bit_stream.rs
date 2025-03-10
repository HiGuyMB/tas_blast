use std::cmp::min;
use crate::error::Result;
use crate::error::ErrorKind::GenericError;

pub struct BitStream {
    data: Vec<u8>,
    bit_offset: u8,
    byte_offset: usize,
}

#[allow(dead_code)]
impl BitStream {
    pub fn new(data: Vec<u8>) -> BitStream {
        BitStream {
            data,
            bit_offset: 0,
            byte_offset: 0,
        }
    }

    pub fn eof(&self) -> bool {
        self.byte_offset >= self.data.len()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    pub fn bytes(&self) -> &Vec<u8> {
        &self.data
    }

    pub fn seek(&mut self, byte_offset: usize, bit_offset: u8) {
        self.byte_offset = byte_offset;
        self.bit_offset = bit_offset;
    }

    pub fn read_bits_u8(&mut self, bits: u8) -> Result<u8> {
        //Sanity
        if bits > 8 {
            return Err(GenericError("Reading too many bits").into());
        }
        //EOF
        if self.byte_offset >= self.data.len()
            || self.byte_offset == self.data.len() - 1 && self.bit_offset + bits > 8
        {
            return Err(GenericError("Read EOF").into());
        }

        let mut result: u8;
        //If this value is going to push us onto the next item we need to do
        // some extra fun math.
        if self.bit_offset + bits >= 8 {
            //How many bits over 8 are we going to need?
            let extra = (self.bit_offset + bits) % 8;
            //How many bits do we have left before 8?
            let remain = bits - extra;

            //Get the first, lower, part of the number, should be stored at the
            // end of the current top. Shift it over so it's in the correct bit
            let first = self.data[self.byte_offset] >> self.bit_offset;
            //Add it to the result
            result = first;
            //Pop the top off because we've used all its bits
            self.byte_offset += 1;

            //If we hit 8 exactly then this will just be extra wasted time. Optimize
            // it out unless we need it.
            if extra > 0 {
                //Get the second, upper, part of the number from the new top and
                // shift it over so it lines up
                let second = (self.data[self.byte_offset] & (0xFF >> (8 - extra))) << remain;
                //Or it with the result so we get the final value
                result |= second;
            }
            //Shift should become however many bits we read from that new top
            self.bit_offset = extra;
        } else {
            //We're not popping anything off so we can just grab the bits from
            // the top and have a nice day.
            result = (self.data[self.byte_offset] >> self.bit_offset) & (0xFF >> (8 - bits));

            //Just add to the shift
            self.bit_offset += bits;
        }
        Ok(result)
    }

    pub fn write_bits_u8(&mut self, value: u8, bits: u8) -> Result<()> {
        //Sanity checking
        if bits > 8 {
            return Err(GenericError("Writing too many bits").into());
        }
        if !(bits == 8 || value < (1 << bits)) {
            return Err(GenericError("Value overflows bit count").into());
        }

        //Sanitize value, don't let it be longer than the number of bits we're promised
        let san_value = value & (0xFF >> (8 - bits));

        if self.data.len() <= self.byte_offset {
            self.data.resize(self.byte_offset + 1, 0);
        }

        let last = self.data.get_mut(self.byte_offset).ok_or(GenericError("Cannot get mutable data"))?;

        //If this value is going to push us onto the next item we need to do
        // some extra fun math.
        if self.bit_offset + bits >= 8 {
            //How many bits over 8 are we going to need?
            let extra = (self.bit_offset + bits) % 8;
            //How many bits do we have left before 8?
            let remain = bits - extra;

            //Get the part of the value that will be pushed onto the current top,
            // should be `remain` bits long.
            let first = san_value & (0xFF >> (8 - remain));
            //Push it on and make sure we start at the next open bit
            *last |= first << self.bit_offset;

            if extra > 0 {
                //Get the second part of the value that will become the next top, should
                // be `extra` bits long.
                let second = (san_value >> remain) & (0xFF >> (8 - extra));
                //Start a new top with it
                self.data.push(second);
            }

            self.byte_offset += 1;

            //Shift should become however many bits long that new top is
            self.bit_offset = extra;
        } else {
            //We don't have to create a new top, we can just slap this one on the
            // end of the original one. OR the bits on, make sure to push them over
            // so they line up, and cut off anything at the end
            *last |= (san_value << self.bit_offset) & (0xFF >> (8 - bits - self.bit_offset));

            //Just add to the shift
            self.bit_offset += bits;
        }
        Ok(())
    }

    pub fn read_bits_u16(&mut self, bits: u8) -> Result<u16> {
        let lower = self.read_bits_u8(min(bits, 8))?;
        if bits <= 8 {
            Ok(u16::from(lower))
        } else {
            let upper = self.read_bits_u8(bits - 8)?;
            Ok(u16::from(lower) | ((u16::from(upper)) << 8u16))
        }
    }

    pub fn read_bits_u32(&mut self, bits: u8) -> Result<u32> {
        let lower = self.read_bits_u16(min(bits, 16))?;
        if bits <= 16 {
            Ok(u32::from(lower))
        } else {
            let upper = self.read_bits_u16(bits - 16)?;
            Ok(u32::from(lower) | ((u32::from(upper)) << 16u32))
        }
    }

    pub fn read_bits_u64(&mut self, bits: u8) -> Result<u64> {
        let lower = self.read_bits_u32(min(bits, 32))?;
        if bits <= 32 {
            Ok(u64::from(lower))
        } else {
            let upper = self.read_bits_u32(bits - 32)?;
            Ok(u64::from(lower) | ((u64::from(upper)) << 32u64))
        }
    }

    pub fn write_bits_u16(&mut self, value: u16, bits: u8) -> Result<()> {
        if !(bits == 16 || value < (1 << u16::from(bits))) {
            return Err(GenericError("Value overflows bit count").into());
        }
        self.write_bits_u8((value & 0xFF) as u8, min(bits, 8))?;
        if bits <= 8 {
            Ok(())
        } else {
            self.write_bits_u8((value >> 8u16) as u8, bits - 8)
        }
    }

    pub fn write_bits_u32(&mut self, value: u32, bits: u8) -> Result<()> {
        if !(bits == 32 || value < (1 << u32::from(bits))) {
            return Err(GenericError("Value overflows bit count").into());
        }
        self.write_bits_u16((value & 0xFF_FF) as u16, min(bits, 16))?;
        if bits <= 16 {
            Ok(())
        } else {
            self.write_bits_u16((value >> 16u32) as u16, bits - 16)
        }
    }

    pub fn write_bits_u64(&mut self, value: u64, bits: u8) -> Result<()> {
        if !(bits == 64 || value < (1 << u64::from(bits))) {
            return Err(GenericError("Value overflows bit count").into());
        }
        self.write_bits_u32((value & 0xFF_FF_FF_FF) as u32, min(bits, 32))?;
        if bits <= 32 {
            Ok(())
        } else {
            self.write_bits_u32((value >> 32u64) as u32, bits - 32)
        }
    }

    pub fn read_bool(&mut self) -> Result<bool> {
        Ok(self.read_bits_u8(1)? == 1)
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        self.read_bits_u8(8)
    }

    pub fn read_u16(&mut self) -> Result<u16> {
        self.read_bits_u16(16)
    }

    pub fn read_u32(&mut self) -> Result<u32> {
        self.read_bits_u32(32)
    }

    pub fn read_u64(&mut self) -> Result<u64> {
        self.read_bits_u64(64)
    }

    pub fn write_bool(&mut self, value: bool) -> Result<()> {
        self.write_bits_u8(value as u8, 1)
    }

    pub fn write_u8(&mut self, value: u8) -> Result<()> {
        self.write_bits_u8(value, 8)
    }

    pub fn write_u16(&mut self, value: u16) -> Result<()> {
        self.write_bits_u16(value, 16)
    }

    pub fn write_u32(&mut self, value: u32) -> Result<()> {
        self.write_bits_u32(value, 32)
    }

    pub fn write_u64(&mut self, value: u64) -> Result<()> {
        self.write_bits_u64(value, 64)
    }

    pub fn read_string(&mut self) -> Result<String> {
        let length = self.read_bits_u8(8)?;
        let mut bytes: Vec<u8> = Vec::with_capacity(length as usize);
        for _ in 0..length {
            bytes.push(self.read_bits_u8(8)?);
        }
        String::from_utf8(bytes).map_err(|e| e.into())
    }

    pub fn write_string(&mut self, value: String) -> Result<()> {
        self.write_bits_u8(value.len() as u8, 8)?;
        for ch in value.into_bytes() {
            self.write_bits_u8(ch, 8)?;
        }
        Ok(())
    }

    pub fn read_optional<T, F>(&mut self, read_fn: F) -> Result<Option<T>>
    where
        F: FnOnce(&mut BitStream) -> Result<T>,
    {
        if self.read_bool()? {
            Ok(Some(read_fn(self)?))
        } else {
            Ok(None)
        }
    }

    pub fn write_optional<T, F>(&mut self, value: Option<T>, write_fn: F) -> Result<()>
    where
        F: Fn(&mut BitStream, T) -> Result<()>,
    {
        match value {
            Some(val) => {
                self.write_bool(true)?;
                write_fn(self, val)
            }
            None => self.write_bool(false),
        }
    }

    pub fn read_scaled_f64_bits(&mut self, bits: u8, scale: f64, offset: f64) -> Result<f64> {
        let inner = self.read_bits_u64(bits)?;
        Ok((inner as f64) * scale + offset)
    }

    pub fn write_scaled_f64_bits(
        &mut self,
        value: f64,
        bits: u8,
        scale: f64,
        offset: f64,
    ) -> Result<()> {
        let scaled = (value - offset) / scale;
//        if !(scaled < f64::from(bits).exp2()) {
//            return Err(());
//        }
        // When importing recs, scaled can sometimes be 65407.999999999993 due to precision
        self.write_bits_u64(scaled.round() as u64, bits)
    }
}
