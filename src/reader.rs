// tiff-core/src/reader.rs
//! TIFF file reading utilities with pluggable data sources
//!
//! This module provides the foundation for reading TIFF files from any data source.
//! The architecture supports automatic decompression and multiple data source types.
//!
//! Architecture:
//! - TiffDataSource: Trait for pluggable data sources (memory, mmap, network, etc.)
//! - InMemorySource: Simple data source for small files loaded into memory  
//! - TiffReader: Generic reader that works with any data source
//! - TiffImageReader: (Future) Higher-level reader with automatic decompression

use crate::{
    error::{Result, TiffError},
    header::{Endian, TiffHeader},
};

/// Trait for TIFF data sources - abstracts where the data comes from
///
/// This allows TiffReader to work with in-memory data, memory-mapped files,
/// network streams, or any other data source by implementing this trait.
///
/// All methods are stateless - they don't modify the data source.
pub trait TiffDataSource {
    /// Get the total size of the data source
    fn len(&self) -> usize;

    /// Check if the data source is empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Read bytes at a specific offset without changing any internal position
    ///
    /// # Arguments
    /// * `offset` - Byte offset to read from
    /// * `count` - Number of bytes to read
    ///
    /// # Returns
    /// Vector containing the read bytes
    ///
    /// # Errors
    /// Returns error if offset + count exceeds data bounds
    fn read_bytes_at(&self, offset: usize, count: usize) -> Result<Vec<u8>>;

    /// Read a single byte at a specific offset
    ///
    /// Default implementation uses read_bytes_at, but data sources can optimize this
    fn read_u8_at(&self, offset: usize) -> Result<u8> {
        let bytes = self.read_bytes_at(offset, 1)?;
        Ok(bytes[0])
    }

    /// Read a u16 at a specific offset with given endianness
    ///
    /// Default implementation uses read_bytes_at, but data sources can optimize this
    fn read_u16_at(&self, offset: usize, endian: Endian) -> Result<u16> {
        let bytes = self.read_bytes_at(offset, 2)?;
        Ok(endian.read_u16([bytes[0], bytes[1]]))
    }

    /// Read a u32 at a specific offset with given endianness
    ///
    /// Default implementation uses read_bytes_at, but data sources can optimize this
    fn read_u32_at(&self, offset: usize, endian: Endian) -> Result<u32> {
        let bytes = self.read_bytes_at(offset, 4)?;
        Ok(endian.read_u32([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

/// In-memory data source - holds data in a `Vec<u8>`
///
/// This is the simplest data source, suitable for small to medium files
/// that can fit entirely in memory. For larger files, consider memory-mapped
/// or streaming data sources.
#[derive(Debug, Clone)]
pub struct InMemorySource {
    data: Vec<u8>,
}

impl InMemorySource {
    /// Create a new in-memory source from a vector of bytes
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Create from a byte slice (copies the data)
    pub fn from_slice(data: &[u8]) -> Self {
        Self::new(data.to_vec())
    }

    /// Get a reference to the underlying data
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }
}

impl TiffDataSource for InMemorySource {
    fn len(&self) -> usize {
        self.data.len()
    }

    fn read_bytes_at(&self, offset: usize, count: usize) -> Result<Vec<u8>> {
        if offset + count > self.data.len() {
            return Err(TiffError::OutOfBounds {
                index: offset + count,
                max: self.data.len(),
            });
        }

        Ok(self.data[offset..offset + count].to_vec())
    }

    // Optimized implementations for primitives (avoid allocation where possible)
    fn read_u8_at(&self, offset: usize) -> Result<u8> {
        if offset + 1 > self.data.len() {
            return Err(TiffError::OutOfBounds {
                index: offset + 1,
                max: self.data.len(),
            });
        }
        Ok(self.data[offset])
    }

    fn read_u16_at(&self, offset: usize, endian: Endian) -> Result<u16> {
        if offset + 2 > self.data.len() {
            return Err(TiffError::OutOfBounds {
                index: offset + 2,
                max: self.data.len(),
            });
        }
        let bytes = [self.data[offset], self.data[offset + 1]];
        Ok(endian.read_u16(bytes))
    }

    fn read_u32_at(&self, offset: usize, endian: Endian) -> Result<u32> {
        if offset + 4 > self.data.len() {
            return Err(TiffError::OutOfBounds {
                index: offset + 4,
                max: self.data.len(),
            });
        }

        let bytes = [
            self.data[offset],
            self.data[offset + 1],
            self.data[offset + 2],
            self.data[offset + 3],
        ];

        Ok(endian.read_u32(bytes))
    }
}

/// Generic TIFF reader that works with any data source
///
/// This reader provides both stateful (position-tracking) and stateless
/// (offset-based) reading operations. The data source is pluggable via
/// the TiffDataSource trait.
///
/// This reader handles the basic TIFF structure (headers, IFDs, tags) but
/// does not handle image decompression. For automatic decompression, use
/// TiffImageReader (future implementation).
#[derive(Debug)]
pub struct TiffReader<T: TiffDataSource> {
    /// The data source (in-memory, memory-mapped, network, etc.)
    source: T,
    /// Current reading position for stateful operations
    position: usize,
}

impl<T: TiffDataSource> TiffReader<T> {
    /// Create a new reader with the given data source
    pub fn new(source: T) -> Self {
        Self {
            source,
            position: 0,
        }
    }

    /// Get the total size of the data
    pub fn len(&self) -> usize {
        self.source.len()
    }

    /// Check if the data is empty
    pub fn is_empty(&self) -> bool {
        self.source.is_empty()
    }

    /// Get the current reading position
    pub fn position(&self) -> usize {
        self.position
    }

    /// Set the reading position
    ///
    /// # Arguments
    /// * `position` - New position to seek to
    ///
    /// # Errors
    /// Returns `OutOfBounds` if position is beyond the end of data
    pub fn seek(&mut self, position: usize) -> Result<()> {
        if position > self.source.len() {
            return Err(TiffError::OutOfBounds {
                index: position,
                max: self.source.len(),
            });
        }
        self.position = position;
        Ok(())
    }

    /// Skip ahead by `count` bytes
    pub fn skip(&mut self, count: usize) -> Result<()> {
        self.seek(self.position + count)
    }

    /// Get remaining bytes from current position
    pub fn remaining(&self) -> usize {
        self.source.len().saturating_sub(self.position)
    }

    /// Check if we're at the end of the data
    pub fn is_at_end(&self) -> bool {
        self.position >= self.source.len()
    }

    // =============================================================================
    // Stateful reading methods (advance position)
    // =============================================================================

    /// Read a single byte and advance position
    pub fn read_u8(&mut self) -> Result<u8> {
        let value = self.source.read_u8_at(self.position)?;
        self.position += 1;
        Ok(value)
    }

    /// Read a u16 and advance position
    pub fn read_u16(&mut self, endian: Endian) -> Result<u16> {
        let value = self.source.read_u16_at(self.position, endian)?;
        self.position += 2;
        Ok(value)
    }

    /// Read a u32 and advance position
    pub fn read_u32(&mut self, endian: Endian) -> Result<u32> {
        let value = self.source.read_u32_at(self.position, endian)?;
        self.position += 4;
        Ok(value)
    }

    /// Read exactly `count` bytes and advance position
    pub fn read_bytes(&mut self, count: usize) -> Result<Vec<u8>> {
        let value = self.source.read_bytes_at(self.position, count)?;
        self.position += count;
        Ok(value)
    }

    // =============================================================================
    // Stateless reading methods (don't change position) - delegate to source
    // =============================================================================

    /// Read a u8 at a specific offset without changing position
    pub fn read_u8_at(&self, offset: usize) -> Result<u8> {
        self.source.read_u8_at(offset)
    }

    /// Read a u16 at a specific offset without changing position
    pub fn read_u16_at(&self, offset: usize, endian: Endian) -> Result<u16> {
        self.source.read_u16_at(offset, endian)
    }

    /// Read a u32 at a specific offset without changing position
    pub fn read_u32_at(&self, offset: usize, endian: Endian) -> Result<u32> {
        self.source.read_u32_at(offset, endian)
    }

    /// Read bytes at a specific offset without changing position
    pub fn read_bytes_at(&self, offset: usize, count: usize) -> Result<Vec<u8>> {
        self.source.read_bytes_at(offset, count)
    }

    // =============================================================================
    // Array reading methods
    // =============================================================================

    /// Read an array of u16s and advance position
    pub fn read_u16_array(&mut self, count: usize, endian: Endian) -> Result<Vec<u16>> {
        let mut result = Vec::with_capacity(count);
        for _ in 0..count {
            result.push(self.read_u16(endian)?);
        }
        Ok(result)
    }

    /// Read an array of u32s and advance position
    pub fn read_u32_array(&mut self, count: usize, endian: Endian) -> Result<Vec<u32>> {
        let mut result = Vec::with_capacity(count);
        for _ in 0..count {
            result.push(self.read_u32(endian)?);
        }
        Ok(result)
    }

    /// Read an array of u16s at a specific offset
    pub fn read_u16_array_at(
        &self,
        offset: usize,
        count: usize,
        endian: Endian,
    ) -> Result<Vec<u16>> {
        let mut result = Vec::with_capacity(count);
        for i in 0..count {
            result.push(self.source.read_u16_at(offset + i * 2, endian)?);
        }
        Ok(result)
    }

    /// Read an array of u32s at a specific offset
    pub fn read_u32_array_at(
        &self,
        offset: usize,
        count: usize,
        endian: Endian,
    ) -> Result<Vec<u32>> {
        let mut result = Vec::with_capacity(count);
        for i in 0..count {
            result.push(self.source.read_u32_at(offset + i * 4, endian)?);
        }
        Ok(result)
    }

    // =============================================================================
    // TIFF-specific convenience methods
    // =============================================================================

    /// Read a TIFF header from the current position and advance
    pub fn read_header(&mut self) -> Result<TiffHeader> {
        let header_bytes = self.read_bytes(TiffHeader::SIZE)?;
        TiffHeader::parse(&header_bytes)
    }

    /// Read a null-terminated ASCII string and advance position
    ///
    /// # Arguments
    /// * `max_length` - Maximum length to read (safety limit)
    ///
    /// # Returns
    /// The string without the null terminator
    pub fn read_ascii_string(&mut self, max_length: usize) -> Result<String> {
        let mut bytes = Vec::new();

        for _ in 0..max_length {
            let byte = self.read_u8()?;
            if byte == 0 {
                // Found null terminator, stop reading
                break;
            }
            bytes.push(byte);
        }

        String::from_utf8(bytes).map_err(|_| TiffError::InvalidString {
            context: "ASCII string".to_string(),
        })
    }

    /// Get access to the underlying data source (for advanced usage)
    pub fn source(&self) -> &T {
        &self.source
    }
}

// =============================================================================
// Future: Image decompression layer
// =============================================================================

// TODO: Add these when ready for decompression support
//
// pub trait Decompressor {
//     fn decompress(&self, data: &[u8]) -> Result<Vec<u8>>;
//     fn name(&self) -> &'static str;
// }
//
// pub struct TiffImageReader<T: TiffDataSource> {
//     reader: TiffReader<T>,
//     decompressor: Box<dyn Decompressor>,
//     compression: Compression,
//     raw_data_mode: bool,
//     // ... layout info
// }
//
// impl<T: TiffDataSource> TiffImageReader<T> {
//     pub fn new(reader: TiffReader<T>, ifd: &ImageFileDirectory) -> Result<Self> {
//         // Automatically detect compression from IFD tags
//         // Create appropriate decompressor
//         // Extract image layout info
//     }
//
//     pub fn with_raw_data(mut self, raw: bool) -> Self { ... }
//     pub fn with_decompressor(mut self, decompressor: Box<dyn Decompressor>) -> Self { ... }
//     pub fn read_strip(&self, strip_index: usize) -> Result<Vec<u8>> { ... }
//     pub fn read_tile(&self, tile_x: u32, tile_y: u32) -> Result<Vec<u8>> { ... }
// }

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_data() -> Vec<u8> {
        vec![
            // TIFF header (8 bytes)
            0x49, 0x49, // "II" - little endian
            0x2A, 0x00, // Magic number 42
            0x08, 0x00, 0x00, 0x00, // IFD offset 8
            // Extra data for testing
            0x12, 0x34, 0x56, 0x78, // 4 more bytes
        ]
    }

    #[test]
    fn test_in_memory_source_creation() {
        let data = create_test_data();
        let source = InMemorySource::new(data.clone());

        assert_eq!(source.len(), data.len());
        assert!(!source.is_empty());
        assert_eq!(source.as_slice(), &data[..]);
    }

    #[test]
    fn test_in_memory_source_bounds_checking() {
        let data = vec![0x01, 0x02];
        let source = InMemorySource::new(data);

        // These should fail due to insufficient data
        assert!(source.read_bytes_at(0, 10).is_err());
        assert!(source.read_u16_at(1, Endian::Little).is_err());
        assert!(source.read_u32_at(0, Endian::Little).is_err());
    }

    #[test]
    fn test_in_memory_source_reading() {
        let data = create_test_data();
        let source = InMemorySource::new(data.clone());

        // Test reading bytes
        let bytes = source.read_bytes_at(0, 4).unwrap();
        assert_eq!(bytes, &data[0..4]);

        // Test reading primitives
        assert_eq!(source.read_u8_at(0).unwrap(), 0x49);
        assert_eq!(source.read_u16_at(0, Endian::Little).unwrap(), 0x4949);
        assert_eq!(source.read_u32_at(4, Endian::Little).unwrap(), 0x00000008);
    }

    #[test]
    fn test_reader_creation() {
        let data = create_test_data();
        let source = InMemorySource::new(data.clone());
        let reader = TiffReader::new(source);

        assert_eq!(reader.len(), data.len());
        assert_eq!(reader.position(), 0);
        assert!(!reader.is_empty());
        assert!(!reader.is_at_end());
    }

    #[test]
    fn test_seeking() {
        let data = create_test_data();
        let source = InMemorySource::new(data);
        let mut reader = TiffReader::new(source);

        assert!(reader.seek(5).is_ok());
        assert_eq!(reader.position(), 5);

        // Seeking past end should fail
        assert!(reader.seek(1000).is_err());

        // Seeking to end should work
        assert!(reader.seek(reader.len()).is_ok());
        assert_eq!(reader.position(), reader.len());
    }

    #[test]
    fn test_stateful_reading() {
        let data = create_test_data();
        let source = InMemorySource::new(data);
        let mut reader = TiffReader::new(source);

        // Read header components
        assert_eq!(reader.read_u8().unwrap(), 0x49);
        assert_eq!(reader.position(), 1);

        assert_eq!(reader.read_u8().unwrap(), 0x49);
        assert_eq!(reader.position(), 2);

        assert_eq!(reader.read_u16(Endian::Little).unwrap(), 42);
        assert_eq!(reader.position(), 4);

        assert_eq!(reader.read_u32(Endian::Little).unwrap(), 8);
        assert_eq!(reader.position(), 8);
    }

    #[test]
    fn test_stateless_reading() {
        let data = create_test_data();
        let source = InMemorySource::new(data);
        let reader = TiffReader::new(source);

        // Position shouldn't change
        assert_eq!(reader.read_u16_at(0, Endian::Little).unwrap(), 0x4949);
        assert_eq!(reader.position(), 0);

        assert_eq!(reader.read_u32_at(4, Endian::Little).unwrap(), 8);
        assert_eq!(reader.position(), 0);
    }

    #[test]
    fn test_header_reading() {
        let data = create_test_data();
        let source = InMemorySource::new(data);
        let mut reader = TiffReader::new(source);

        let header = reader.read_header().unwrap();
        assert_eq!(header.endianness(), Endian::Little);
        assert_eq!(header.magic, 42);
        assert_eq!(header.ifd_offset, 8);
        assert_eq!(reader.position(), 8); // Should advance position
    }

    #[test]
    fn test_array_reading() {
        let data = vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let source = InMemorySource::new(data);
        let mut reader = TiffReader::new(source);

        let values = reader.read_u16_array(3, Endian::Big).unwrap();
        assert_eq!(values, vec![0x1234, 0x5678, 0x9ABC]);
        assert_eq!(reader.position(), 6);
    }

    #[test]
    fn test_endian_conversion() {
        // Test our endian conversion works correctly
        let data = vec![0x12, 0x34, 0x56, 0x78];
        let source = InMemorySource::new(data);

        // Little endian: 0x3412
        assert_eq!(source.read_u16_at(0, Endian::Little).unwrap(), 0x3412);

        // Big endian: 0x1234
        assert_eq!(source.read_u16_at(0, Endian::Big).unwrap(), 0x1234);

        // Little endian u32: 0x78563412
        assert_eq!(source.read_u32_at(0, Endian::Little).unwrap(), 0x78563412);

        // Big endian u32: 0x12345678
        assert_eq!(source.read_u32_at(0, Endian::Big).unwrap(), 0x12345678);
    }

    #[test]
    fn test_skip_functionality() {
        let data = create_test_data();
        let source = InMemorySource::new(data);
        let mut reader = TiffReader::new(source);

        // Test skipping
        assert!(reader.skip(3).is_ok());
        assert_eq!(reader.position(), 3);

        assert!(reader.skip(2).is_ok());
        assert_eq!(reader.position(), 5);

        // Skip past end should fail
        assert!(reader.skip(1000).is_err());
    }

    #[test]
    fn test_ascii_string_reading() {
        let data = b"Hello\0World\0Extra".to_vec();
        let source = InMemorySource::new(data);
        let mut reader = TiffReader::new(source);

        let string1 = reader.read_ascii_string(10).unwrap();
        assert_eq!(string1, "Hello");
        assert_eq!(reader.position(), 6); // "Hello\0" = 6 bytes

        let string2 = reader.read_ascii_string(10).unwrap();
        assert_eq!(string2, "World");
        assert_eq!(reader.position(), 12); // Previous 6 + "World\0" = 12 bytes
    }

    #[test]
    fn test_array_reading_at_offset() {
        let data = vec![0xFF, 0xFF, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let source = InMemorySource::new(data);
        let reader = TiffReader::new(source);

        // Read 2 u16s starting at offset 2, skipping the 0xFF bytes
        let values = reader.read_u16_array_at(2, 2, Endian::Big).unwrap();
        assert_eq!(values, vec![0x1234, 0x5678]);
        assert_eq!(reader.position(), 0); // Position should be unchanged

        // Read 1 u32 starting at offset 2
        let value = reader.read_u32_array_at(2, 1, Endian::Big).unwrap();
        assert_eq!(value, vec![0x12345678]);
    }
}
