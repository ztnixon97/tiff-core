//! Pure Rust TIFF format implementation
//! 
//! This crate provides a from-scratch implementation of the TIFF file format
//! with no external dependencies. It supports reading TIFF files of various
//! formats and will eventually support GeoTIFF extensions.
//!
//! # Architecture
//! 
//! The crate is organized into several modules:
//! - `reader`: Pluggable data sources and reading utilities
//! - `header`: TIFF header parsing and endianness handling
//! - `ifd`: Image File Directory parsing and tag value extraction
//! - `tags`: Standard TIFF tag definitions and enums
//! - `error`: Error types and handling
//!
//! # Basic Usage
//!
//! ```rust
//! use tiff_core::{TiffFile, InMemorySource};
//! 
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Read a TIFF file
//! let data = std::fs::read("image.tif")?;
//! let tiff = TiffFile::from_bytes(data)?;
//!
//! // Get image information
//! if let Some(summary) = tiff.main_image_info()? {
//!     println!("Image: {}", summary.description());
//! }
//! # Ok(())
//! # }
//! ```

#![deny(missing_docs)]
#![warn(rust_2018_idioms)]

pub mod error;
pub mod header;
pub mod reader;
pub mod ifd;
pub mod tags;

// Re-export commonly used types for convenience
pub use error::{TiffError, Result};
pub use header::{Endian, TiffHeader};
pub use reader::{TiffDataSource, TiffReader, InMemorySource};
pub use ifd::{ImageFileDirectory, IfdEntry, TagValue, FieldType, ImageSummary};
pub use tags::{
    Compression, PhotometricInterpretation, ResolutionUnit, SampleFormat,
    tag_name, is_required_tag, is_layout_tag, is_data_location_tag,
};

/// The main TIFF file structure
/// 
/// This represents a complete TIFF file with header and all IFDs.
/// Most users should use this as the high-level interface.
#[derive(Debug)]
pub struct TiffFile<T: TiffDataSource> {
    /// The underlying reader
    pub reader: TiffReader<T>,
    /// File header
    pub header: TiffHeader,
    /// All Image File Directories in the file
    pub ifds: Vec<ImageFileDirectory>,
}

impl<T: TiffDataSource> TiffFile<T> {
    /// Read a TIFF file from a data source
    /// 
    /// This is the main entry point for parsing TIFF files.
    pub fn from_reader(mut reader: TiffReader<T>) -> Result<Self> {
        // Read header first
        let header = reader.read_header()?;
        
        // Read all IFDs
        let mut ifds = Vec::new();
        let mut ifd_offset = header.ifd_offset as usize;
        
        while ifd_offset != 0 {
            let ifd = reader.read_ifd(ifd_offset, header.endianness())?;
            ifd_offset = ifd.next_ifd_offset;
            ifds.push(ifd);
        }
        
        Ok(TiffFile { reader, header, ifds })
    }

    /// Get the number of images (IFDs) in this file
    pub fn image_count(&self) -> usize {
        self.ifds.len()
    }

    /// Get a specific IFD by index
    pub fn get_ifd(&self, index: usize) -> Option<&ImageFileDirectory> {
        self.ifds.get(index)
    }

    /// Get the first (main) image's IFD
    pub fn main_ifd(&self) -> Option<&ImageFileDirectory> {
        self.ifds.first()
    }

    /// Get the endianness of this TIFF file
    pub fn endianness(&self) -> Endian {
        self.header.endianness()
    }

    /// Get basic image information from the main IFD
    pub fn main_image_info(&self) -> Result<Option<ImageSummary>> {
        if let Some(ifd) = self.main_ifd() {
            Ok(Some(ifd.image_summary(&self.reader, self.endianness())?))
        } else {
            Ok(None)
        }
    }

    /// Get image information for all IFDs
    pub fn all_image_info(&self) -> Result<Vec<ImageSummary>> {
        let mut summaries = Vec::with_capacity(self.ifds.len());
        for ifd in &self.ifds {
            summaries.push(ifd.image_summary(&self.reader, self.endianness())?);
        }
        Ok(summaries)
    }

    /// Check if this is a valid TIFF file
    pub fn is_valid(&self) -> Result<bool> {
        if self.ifds.is_empty() {
            return Ok(false);
        }
        
        // Check if the main IFD is valid
        if let Some(main_ifd) = self.main_ifd() {
            main_ifd.is_valid_tiff(&self.reader, self.endianness())
        } else {
            Ok(false)
        }
    }
}

impl TiffFile<InMemorySource> {
    /// Create from in-memory data
    /// 
    /// Convenience method for the common case of loading a file into memory.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        let source = InMemorySource::new(data);
        let reader = TiffReader::new(source);
        Self::from_reader(reader)
    }
}