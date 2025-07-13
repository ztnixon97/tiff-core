// tiff-core/src/header.rs
//! TIFF header structures and parsing

use crate::{TiffError, Result};

/// Byte order (endianness) of the TIFF file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    /// Little-endian byte order (Intel format) - "II"
    Little,
    /// Big-endian byte order (Motorola format) - "MM"
    Big,
}

/// TIFF file header (first 8 bytes of every TIFF file)
#[derive(Debug, Clone)]
pub struct TiffHeader {
    /// Byte order indicator
    pub endian: Endian,
    /// Magic number (should always be 42)
    pub magic: u16,
    /// Offset to the first Image File Directory
    pub ifd_offset: u32,
}

impl TiffHeader {
    /// The size of a TIFF header in bytes
    pub const SIZE: usize = 8;
    
    /// The expected magic number in TIFF files (42 - Answer to Life, Universe, and Everything!)
    pub const MAGIC_NUMBER: u16 = 42;
    
    /// Parse a TIFF header from the first 8 bytes of data
    /// 
    /// # Arguments
    /// * `data` - Byte slice containing at least 8 bytes
    /// 
    /// # Returns
    /// * `Ok(TiffHeader)` if parsing succeeds
    /// * `Err(TiffError)` if data is invalid or insufficient
    pub fn parse(data: &[u8]) -> Result<Self> {
        // Check if we have enough bytes for a complete header
        if data.len() < Self::SIZE {
            return Err(TiffError::InsufficientData {
                operation: "reading TIFF header",
                needed: Self::SIZE,
                available: data.len(),
            });
        }
        
        // Parse byte order from first 2 bytes
        let endian = Endian::from_bytes(&data[0..2])?;
        
        // Parse magic number from bytes 2-3 using the detected endianness
        let magic_bytes = [data[2], data[3]];
        let magic = endian.read_u16(magic_bytes);
        
        // Validate magic number
        if magic != Self::MAGIC_NUMBER {
            return Err(TiffError::InvalidMagic { found: magic });
        }
        
        // Parse IFD offset from bytes 4-7 using the detected endianness
        let ifd_offset_bytes = [data[4], data[5], data[6], data[7]];
        let ifd_offset = endian.read_u32(ifd_offset_bytes);
        
        Ok(TiffHeader {
            endian,
            magic,
            ifd_offset,
        })
    }
    
    /// Get the endianness of this TIFF file
    pub fn endianness(&self) -> Endian {
        self.endian
    }
    
    /// Check if this TIFF file uses little-endian byte order
    pub fn is_little_endian(&self) -> bool {
        self.endian == Endian::Little
    }
    
    /// Check if this TIFF file uses big-endian byte order  
    pub fn is_big_endian(&self) -> bool {
        self.endian == Endian::Big
    }
}

impl Endian {
    /// Parse endianness from the first 2 bytes of TIFF data
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 2 {
            return Err(TiffError::InsufficientData {
                operation: "reading byte order",
                needed: 2,
                available: bytes.len(),
            });
        }
        
        match &bytes[0..2] {
            b"II" => Ok(Endian::Little),  // Intel (little-endian)
            b"MM" => Ok(Endian::Big),     // Motorola (big-endian)
            _ => {
                let found = [bytes[0], bytes[1]];
                Err(TiffError::InvalidByteOrder { found })
            }
        }
    }
    
    /// Convert a 2-byte array to u16 using this endianness
    pub fn read_u16(self, bytes: [u8; 2]) -> u16 {
        match self {
            Endian::Little => u16::from_le_bytes(bytes),
            Endian::Big => u16::from_be_bytes(bytes),
        }
    }
    
    /// Convert a 4-byte array to u32 using this endianness
    pub fn read_u32(self, bytes: [u8; 4]) -> u32 {
        match self {
            Endian::Little => u32::from_le_bytes(bytes),
            Endian::Big => u32::from_be_bytes(bytes),
        }
    }

    /// Convert an 8-byte array to u64 using this endianness
    pub fn read_u64(self, bytes: [u8; 8]) -> u64 {
        match self {
            Endian::Little => u64::from_le_bytes(bytes),
            Endian::Big => u64::from_be_bytes(bytes),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_size_constant() {
        assert_eq!(TiffHeader::SIZE, 8);
        assert_eq!(TiffHeader::MAGIC_NUMBER, 42);
    }
    
    #[test]
    fn test_little_endian_header() {
        // Little-endian TIFF: "II" + 42 + offset 8
        let data = [0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00];
        
        let header = TiffHeader::parse(&data).unwrap();
        assert_eq!(header.endian, Endian::Little);
        assert_eq!(header.magic, 42);
        assert_eq!(header.ifd_offset, 8);
        assert!(header.is_little_endian());
        assert!(!header.is_big_endian());
    }
    
    #[test]
    fn test_big_endian_header() {
        // Big-endian TIFF: "MM" + 42 + offset 8  
        let data = [0x4D, 0x4D, 0x00, 0x2A, 0x00, 0x00, 0x00, 0x08];
        
        let header = TiffHeader::parse(&data).unwrap();
        assert_eq!(header.endian, Endian::Big);
        assert_eq!(header.magic, 42);
        assert_eq!(header.ifd_offset, 8);
        assert!(!header.is_little_endian());
        assert!(header.is_big_endian());
    }
    
    #[test]
    fn test_endian_read_methods() {
        // Test little-endian reading
        let little = Endian::Little;
        assert_eq!(little.read_u16([0x34, 0x12]), 0x1234);
        assert_eq!(little.read_u32([0x78, 0x56, 0x34, 0x12]), 0x12345678);
        
        // Test big-endian reading
        let big = Endian::Big;
        assert_eq!(big.read_u16([0x12, 0x34]), 0x1234);
        assert_eq!(big.read_u32([0x12, 0x34, 0x56, 0x78]), 0x12345678);
    }
    
    #[test]
    fn test_insufficient_data() {
        let data = [0x49, 0x49, 0x2A]; // Only 3 bytes
        
        let result = TiffHeader::parse(&data);
        assert!(result.is_err());
        
        if let Err(TiffError::InsufficientData { operation, needed, available }) = result {
            assert_eq!(operation, "reading TIFF header");
            assert_eq!(needed, 8);
            assert_eq!(available, 3);
        } else {
            panic!("Expected InsufficientData error");
        }
    }
    
    #[test]
    fn test_invalid_magic() {
        // Valid endian but wrong magic number (43 instead of 42)
        let data = [0x49, 0x49, 0x2B, 0x00, 0x08, 0x00, 0x00, 0x00];
        
        let result = TiffHeader::parse(&data);
        assert!(result.is_err());
        
        if let Err(TiffError::InvalidMagic { found }) = result {
            assert_eq!(found, 43);
        } else {
            panic!("Expected InvalidMagic error");
        }
    }
    
    #[test]
    fn test_invalid_byte_order() {
        // Invalid byte order indicator
        let data = [0x58, 0x58, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00];
        
        let result = TiffHeader::parse(&data);
        assert!(result.is_err());
        
        if let Err(TiffError::InvalidByteOrder { found }) = result {
            assert_eq!(found, [0x58, 0x58]);
        } else {
            panic!("Expected InvalidByteOrder error");
        }
    }
    
    #[test]
    fn test_zero_ifd_offset() {
        // Valid header but with IFD offset of 0 (unusual but technically valid)
        let data = [0x49, 0x49, 0x2A, 0x00, 0x00, 0x00, 0x00, 0x00];
        
        let header = TiffHeader::parse(&data).unwrap();
        assert_eq!(header.ifd_offset, 0);
    }
}