// tiff-core/src/ifd.rs
//! Image File Directory structures and parsing
//!
//! IFDs contain all the metadata about a TIFF image - dimensions, compression,
//! where the actual image data is stored, etc. Each IFD contains a series of
//! 12-byte entries that describe different aspects of the image.

use crate::{TiffError, Result};
use crate::header::Endian;
use crate::reader::{TiffReader, TiffDataSource};
use crate::tags::{self, Compression, PhotometricInterpretation, ResolutionUnit, SampleFormat};

/// An Image File Directory entry (12 bytes)
/// 
/// Each entry describes one piece of metadata about the image.
/// The structure is always the same, but the interpretation depends
/// on the tag and field type.
#[derive(Debug, Clone)]
pub struct IfdEntry {
    /// The tag identifier (what kind of data this is)
    /// Examples: 256 = ImageWidth, 257 = ImageLength, 259 = Compression
    pub tag: u16,
    
    /// The data type (byte, short, long, rational, etc.)
    /// This determines how to interpret the value_offset field
    pub field_type: u16,
    
    /// Number of values of this type
    /// Examples: 1 for a single width value, 3 for RGB bits per sample
    pub count: u32,
    
    /// Either the value itself (if ≤ 4 bytes) or offset to the value
    /// This is the tricky part - depends on field_type and count
    pub value_offset: u32,
}

/// Data types used in TIFF tags
/// 
/// These correspond to the field_type values in IFD entries.
/// Each type has a specific byte size and interpretation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FieldType {
    /// 8-bit unsigned integer
    Byte = 1,
    /// ASCII string (null-terminated)
    Ascii = 2,
    /// 16-bit unsigned integer
    Short = 3,
    /// 32-bit unsigned integer
    Long = 4,
    /// Rational (two longs: numerator, denominator)
    Rational = 5,
    /// 8-bit signed integer
    SByte = 6,
    /// Undefined (8-bit byte that can contain anything)
    Undefined = 7,
    /// 16-bit signed integer
    SShort = 8,
    /// 32-bit signed integer
    SLong = 9,
    /// Signed rational (two slongs)
    SRational = 10,
    /// 32-bit IEEE floating point
    Float = 11,
    /// 64-bit IEEE floating point
    Double = 12,
}

impl FieldType {
    /// Convert from u16 to FieldType
    pub fn from_u16(value: u16) -> Result<Self> {
        match value {
            1 => Ok(FieldType::Byte),
            2 => Ok(FieldType::Ascii),
            3 => Ok(FieldType::Short),
            4 => Ok(FieldType::Long),
            5 => Ok(FieldType::Rational),
            6 => Ok(FieldType::SByte),
            7 => Ok(FieldType::Undefined),
            8 => Ok(FieldType::SShort),
            9 => Ok(FieldType::SLong),
            10 => Ok(FieldType::SRational),
            11 => Ok(FieldType::Float),
            12 => Ok(FieldType::Double),
            _ => Err(TiffError::InvalidFieldType { found: value }),
        }
    }

    /// Get the size in bytes of this data type
    pub fn byte_size(self) -> usize {
        match self {
            FieldType::Byte | FieldType::SByte | FieldType::Ascii | FieldType::Undefined => 1,
            FieldType::Short | FieldType::SShort => 2,
            FieldType::Long | FieldType::SLong | FieldType::Float => 4,
            FieldType::Rational | FieldType::SRational | FieldType::Double => 8,
        }
    }
}

/// Summary of image information extracted from an IFD
/// 
/// This provides a convenient overview of the key image properties
/// without having to call multiple methods.
#[derive(Debug, Clone)]
pub struct ImageSummary {
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// Number of samples (channels) per pixel
    pub samples_per_pixel: u32,
    /// Bits per sample for each channel
    pub bits_per_sample: Vec<u32>,
    /// Compression method used
    pub compression: Compression,
    /// Color interpretation
    pub photometric_interpretation: Option<PhotometricInterpretation>,
    /// Whether the image uses tiled layout
    pub is_tiled: bool,
}

impl ImageSummary {
    /// Calculate total bits per pixel
    pub fn bits_per_pixel(&self) -> u32 {
        self.bits_per_sample.iter().sum()
    }

    /// Calculate bytes per pixel (rounded up)
    pub fn bytes_per_pixel(&self) -> u32 {
        (self.bits_per_pixel() + 7) / 8
    }

    /// Check if this is a grayscale image
    pub fn is_grayscale(&self) -> bool {
        self.samples_per_pixel == 1 || 
        matches!(self.photometric_interpretation, 
                Some(PhotometricInterpretation::BlackIsZero) | 
                Some(PhotometricInterpretation::WhiteIsZero))
    }

    /// Check if this is an RGB image
    pub fn is_rgb(&self) -> bool {
        self.samples_per_pixel >= 3 &&
        matches!(self.photometric_interpretation, Some(PhotometricInterpretation::Rgb))
    }

    /// Check if this image has an alpha channel
    pub fn has_alpha(&self) -> bool {
        self.samples_per_pixel == 2 && self.is_grayscale() ||  // Grayscale + Alpha
        self.samples_per_pixel == 4 && self.is_rgb()           // RGB + Alpha
    }

    /// Get a human-readable description
    pub fn description(&self) -> String {
        let color_desc = match self.photometric_interpretation {
            Some(PhotometricInterpretation::Rgb) => {
                if self.has_alpha() { "RGBA" } else { "RGB" }
            }
            Some(PhotometricInterpretation::BlackIsZero) | 
            Some(PhotometricInterpretation::WhiteIsZero) => {
                if self.has_alpha() { "Grayscale+Alpha" } else { "Grayscale" }
            }
            Some(PhotometricInterpretation::Palette) => "Palette",
            Some(PhotometricInterpretation::Cmyk) => "CMYK",
            _ => "Unknown",
        };

        let layout = if self.is_tiled { "tiled" } else { "stripped" };
        
        format!(
            "{}x{} {} {}-bit {} ({:?})",
            self.width, 
            self.height, 
            color_desc,
            self.bits_per_pixel(),
            layout,
            self.compression
        )
    }
}

/// The value stored in a TIFF tag
/// 
/// Different tags store different types of data. This enum represents
/// all the possible value types that can be stored in TIFF tags.
#[derive(Debug, Clone)]
pub enum TagValue {
    /// Unsigned 8-bit integers
    Bytes(Vec<u8>),
    /// ASCII string (without null terminator)
    Ascii(String),
    /// Unsigned 16-bit integers
    Shorts(Vec<u16>),
    /// Unsigned 32-bit integers
    Longs(Vec<u32>),
    /// Rational numbers (numerator/denominator pairs)
    Rationals(Vec<(u32, u32)>),
    /// Signed 8-bit integers
    SBytes(Vec<i8>),
    /// Undefined bytes (raw data)
    Undefined(Vec<u8>),
    /// Signed 16-bit integers
    SShorts(Vec<i16>),
    /// Signed 32-bit integers
    SLongs(Vec<i32>),
    /// Signed rational numbers
    SRationals(Vec<(i32, i32)>),
    /// 32-bit floating point
    Floats(Vec<f32>),
    /// 64-bit floating point
    Doubles(Vec<f64>),
}

impl TagValue {
    /// Try to get the first value as a u32 (common case for single values)
    pub fn as_u32(&self) -> Option<u32> {
        match self {
            TagValue::Shorts(v) if !v.is_empty() => Some(v[0] as u32),
            TagValue::Longs(v) if !v.is_empty() => Some(v[0]),
            TagValue::Bytes(v) if !v.is_empty() => Some(v[0] as u32),
            _ => None,
        }
    }

    /// Try to get the first value as a u16 (common case)
    pub fn as_u16(&self) -> Option<u16> {
        match self {
            TagValue::Shorts(v) if !v.is_empty() => Some(v[0]),
            TagValue::Bytes(v) if !v.is_empty() => Some(v[0] as u16),
            _ => None,
        }
    }

    /// Try to get as a string
    pub fn as_string(&self) -> Option<&str> {
        match self {
            TagValue::Ascii(s) => Some(s),
            _ => None,
        }
    }

    /// Try to get as a vec of u32s
    pub fn as_u32_vec(&self) -> Option<Vec<u32>> {
        match self {
            TagValue::Longs(v) => Some(v.clone()),
            TagValue::Shorts(v) => Some(v.iter().map(|&x| x as u32).collect()),
            _ => None,
        }
    }

    /// Try to get the first value as an i32 (for signed types)
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            TagValue::SLongs(v) if !v.is_empty() => Some(v[0]),
            TagValue::SShorts(v) if !v.is_empty() => Some(v[0] as i32),
            TagValue::SBytes(v) if !v.is_empty() => Some(v[0] as i32),
            _ => None,
        }
    }

    /// Try to get the first value as f32
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            TagValue::Floats(v) if !v.is_empty() => Some(v[0]),
            TagValue::Doubles(v) if !v.is_empty() => Some(v[0] as f32),
            _ => None,
        }
    }

    /// Try to get the first value as f64
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            TagValue::Doubles(v) if !v.is_empty() => Some(v[0]),
            TagValue::Floats(v) if !v.is_empty() => Some(v[0] as f64),
            _ => None,
        }
    }

    /// Try to get the first rational as a floating point value
    pub fn as_rational_f64(&self) -> Option<f64> {
        match self {
            TagValue::Rationals(v) if !v.is_empty() => {
                let (num, den) = v[0];
                if den != 0 {
                    Some(num as f64 / den as f64)
                } else {
                    None
                }
            }
            TagValue::SRationals(v) if !v.is_empty() => {
                let (num, den) = v[0];
                if den != 0 {
                    Some(num as f64 / den as f64)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// An Image File Directory containing tag entries
/// 
/// This represents one "page" or "image" in a TIFF file. Multi-page
/// TIFFs have multiple IFDs linked together.
#[derive(Debug, Clone)]
pub struct ImageFileDirectory {
    /// The IFD entries (tags)
    pub entries: Vec<IfdEntry>,
    /// Offset to the next IFD (0 if this is the last one)
    pub next_ifd_offset: usize,
}

impl ImageFileDirectory {
    /// Find an entry by tag number
    pub fn find_entry(&self, tag: u16) -> Option<&IfdEntry> {
        self.entries.iter().find(|entry| entry.tag == tag)
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the IFD is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get a parsed tag value by tag number
    /// 
    /// This is a convenience method that finds the entry and parses its value.
    pub fn get_tag_value<T: TiffDataSource>(
        &self, 
        tag: u16, 
        reader: &TiffReader<T>, 
        endian: Endian
    ) -> Result<Option<TagValue>> {
        if let Some(entry) = self.find_entry(tag) {
            Ok(Some(reader.parse_tag_value(entry, endian)?))
        } else {
            Ok(None)
        }
    }

    // =============================================================================
    // Basic image information convenience methods
    // =============================================================================

    /// Get image width in pixels
    pub fn image_width<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<u32>> {
        Ok(self.get_tag_value(tags::tags::IMAGE_WIDTH, reader, endian)?
            .and_then(|v| v.as_u32()))
    }

    /// Get image height in pixels
    pub fn image_height<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<u32>> {
        Ok(self.get_tag_value(tags::tags::IMAGE_LENGTH, reader, endian)?
            .and_then(|v| v.as_u32()))
    }

    /// Get bits per sample (per channel)
    pub fn bits_per_sample<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<Vec<u32>>> {
        Ok(self.get_tag_value(tags::tags::BITS_PER_SAMPLE, reader, endian)?
            .and_then(|v| v.as_u32_vec()))
    }

    /// Get samples (channels) per pixel
    pub fn samples_per_pixel<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<u32>> {
        Ok(self.get_tag_value(tags::tags::SAMPLES_PER_PIXEL, reader, endian)?
            .and_then(|v| v.as_u32()))
    }

    /// Get compression type
    pub fn compression<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<Compression>> {
        Ok(self.get_tag_value(tags::tags::COMPRESSION, reader, endian)?
            .and_then(|v| v.as_u32())
            .and_then(Compression::from_u32))
    }

    /// Get photometric interpretation
    pub fn photometric_interpretation<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<PhotometricInterpretation>> {
        Ok(self.get_tag_value(tags::tags::PHOTOMETRIC_INTERPRETATION, reader, endian)?
            .and_then(|v| v.as_u32())
            .and_then(PhotometricInterpretation::from_u32))
    }

    /// Get sample format
    pub fn sample_format<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<SampleFormat>> {
        Ok(self.get_tag_value(tags::tags::SAMPLE_FORMAT, reader, endian)?
            .and_then(|v| v.as_u32())
            .and_then(SampleFormat::from_u32))
    }

    // =============================================================================
    // Image data organization convenience methods
    // =============================================================================

    /// Get strip offsets (where image data is stored)
    pub fn strip_offsets<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<Vec<u32>>> {
        Ok(self.get_tag_value(tags::tags::STRIP_OFFSETS, reader, endian)?
            .and_then(|v| v.as_u32_vec()))
    }

    /// Get strip byte counts (how much data per strip)
    pub fn strip_byte_counts<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<Vec<u32>>> {
        Ok(self.get_tag_value(tags::tags::STRIP_BYTE_COUNTS, reader, endian)?
            .and_then(|v| v.as_u32_vec()))
    }

    /// Get rows per strip
    pub fn rows_per_strip<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<u32>> {
        Ok(self.get_tag_value(tags::tags::ROWS_PER_STRIP, reader, endian)?
            .and_then(|v| v.as_u32()))
    }

    /// Get tile width (for tiled images)
    pub fn tile_width<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<u32>> {
        Ok(self.get_tag_value(tags::tags::TILE_WIDTH, reader, endian)?
            .and_then(|v| v.as_u32()))
    }

    /// Get tile height (for tiled images)
    pub fn tile_height<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<u32>> {
        Ok(self.get_tag_value(tags::tags::TILE_LENGTH, reader, endian)?
            .and_then(|v| v.as_u32()))
    }

    /// Get tile offsets (for tiled images)
    pub fn tile_offsets<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<Vec<u32>>> {
        Ok(self.get_tag_value(tags::tags::TILE_OFFSETS, reader, endian)?
            .and_then(|v| v.as_u32_vec()))
    }

    /// Get tile byte counts (for tiled images)
    pub fn tile_byte_counts<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<Vec<u32>>> {
        Ok(self.get_tag_value(tags::tags::TILE_BYTE_COUNTS, reader, endian)?
            .and_then(|v| v.as_u32_vec()))
    }

    /// Check if this image uses tiled layout (vs strip layout)
    pub fn is_tiled<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<bool> {
        Ok(self.tile_width(reader, endian)?.is_some())
    }

    // =============================================================================
    // Resolution convenience methods
    // =============================================================================

    /// Get X resolution (horizontal)
    pub fn x_resolution<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<f64>> {
        Ok(self.get_tag_value(tags::tags::X_RESOLUTION, reader, endian)?
            .and_then(|v| v.as_rational_f64()))
    }

    /// Get Y resolution (vertical)
    pub fn y_resolution<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<f64>> {
        Ok(self.get_tag_value(tags::tags::Y_RESOLUTION, reader, endian)?
            .and_then(|v| v.as_rational_f64()))
    }

    /// Get resolution unit
    pub fn resolution_unit<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<ResolutionUnit>> {
        Ok(self.get_tag_value(tags::tags::RESOLUTION_UNIT, reader, endian)?
            .and_then(|v| v.as_u32())
            .and_then(ResolutionUnit::from_u32))
    }

    // =============================================================================
    // Metadata convenience methods
    // =============================================================================

    /// Get image description
    pub fn image_description<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<String>> {
        Ok(self.get_tag_value(tags::tags::IMAGE_DESCRIPTION, reader, endian)?
            .and_then(|v| v.as_string().map(|s| s.to_string())))
    }

    /// Get make/manufacturer
    pub fn make<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<String>> {
        Ok(self.get_tag_value(tags::tags::MAKE, reader, endian)?
            .and_then(|v| v.as_string().map(|s| s.to_string())))
    }

    /// Get model
    pub fn model<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<String>> {
        Ok(self.get_tag_value(tags::tags::MODEL, reader, endian)?
            .and_then(|v| v.as_string().map(|s| s.to_string())))
    }

    /// Get software used to create the image
    pub fn software<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<String>> {
        Ok(self.get_tag_value(tags::tags::SOFTWARE, reader, endian)?
            .and_then(|v| v.as_string().map(|s| s.to_string())))
    }

    /// Get creation date/time
    pub fn date_time<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<String>> {
        Ok(self.get_tag_value(tags::tags::DATE_TIME, reader, endian)?
            .and_then(|v| v.as_string().map(|s| s.to_string())))
    }

    /// Get artist/photographer
    pub fn artist<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<String>> {
        Ok(self.get_tag_value(tags::tags::ARTIST, reader, endian)?
            .and_then(|v| v.as_string().map(|s| s.to_string())))
    }

    /// Get copyright notice
    pub fn copyright<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<Option<String>> {
        Ok(self.get_tag_value(tags::tags::COPYRIGHT, reader, endian)?
            .and_then(|v| v.as_string().map(|s| s.to_string())))
    }

    // =============================================================================
    // Validation and summary methods
    // =============================================================================

    /// Check if this IFD has all required tags for a valid TIFF
    pub fn is_valid_tiff<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<bool> {
        let has_width = self.image_width(reader, endian)?.is_some();
        let has_height = self.image_height(reader, endian)?.is_some();
        let has_strips = self.strip_offsets(reader, endian)?.is_some();
        let has_strip_counts = self.strip_byte_counts(reader, endian)?.is_some();
        let has_tiles = self.tile_offsets(reader, endian)?.is_some();
        let has_tile_counts = self.tile_byte_counts(reader, endian)?.is_some();

        // Must have width and height
        // Must have either (strips + strip counts) OR (tiles + tile counts)
        Ok(has_width && has_height && 
           ((has_strips && has_strip_counts) || (has_tiles && has_tile_counts)))
    }

    /// Get a summary of the image described by this IFD
    pub fn image_summary<T: TiffDataSource>(&self, reader: &TiffReader<T>, endian: Endian) -> Result<ImageSummary> {
        let width = self.image_width(reader, endian)?.unwrap_or(0);
        let height = self.image_height(reader, endian)?.unwrap_or(0);
        let samples_per_pixel = self.samples_per_pixel(reader, endian)?.unwrap_or(1);
        let bits_per_sample = self.bits_per_sample(reader, endian)?
            .unwrap_or_else(|| vec![1; samples_per_pixel as usize]);
        let compression = self.compression(reader, endian)?.unwrap_or(Compression::None);
        let photometric = self.photometric_interpretation(reader, endian)?;
        let is_tiled = self.is_tiled(reader, endian)?;

        Ok(ImageSummary {
            width,
            height,
            samples_per_pixel,
            bits_per_sample,
            compression,
            photometric_interpretation: photometric,
            is_tiled,
        })
    }
}

/// Extension methods for TiffReader to handle IFD parsing
impl<T: TiffDataSource> TiffReader<T> {
    /// Read an IFD (Image File Directory) at the given offset
    /// 
    /// # Arguments
    /// * `offset` - Byte offset where the IFD starts
    /// * `endian` - Byte order to use for reading
    /// 
    /// # Returns
    /// Parsed IFD with all entries and next IFD offset
    pub fn read_ifd(&mut self, offset: usize, endian: Endian) -> Result<ImageFileDirectory> {
        // Seek to the IFD location
        self.seek(offset)?;

        // Read number of directory entries (2 bytes)
        let num_entries = self.read_u16(endian)?;
        
        let mut entries = Vec::with_capacity(num_entries as usize);
        
        // Read each IFD entry (12 bytes each)
        for _ in 0..num_entries {
            let entry = self.read_ifd_entry(endian)?;
            entries.push(entry);
        }

        // Read offset to next IFD (4 bytes)
        let next_ifd_offset = self.read_u32(endian)? as usize;

        Ok(ImageFileDirectory {
            entries,
            next_ifd_offset,
        })
    }

    /// Read a single IFD entry (12 bytes)
    fn read_ifd_entry(&mut self, endian: Endian) -> Result<IfdEntry> {
        let tag = self.read_u16(endian)?;
        let field_type = self.read_u16(endian)?;
        let count = self.read_u32(endian)?;
        let value_offset = self.read_u32(endian)?;

        Ok(IfdEntry {
            tag,
            field_type,
            count,
            value_offset,
        })
    }

    /// Parse the actual value from an IFD entry
    /// 
    /// This is where the magic happens - determining whether the value
    /// is stored inline or at an offset, and parsing it according to
    /// the field type.
    pub fn parse_tag_value(&self, entry: &IfdEntry, endian: Endian) -> Result<TagValue> {
        let field_type = FieldType::from_u16(entry.field_type)?;
        let total_bytes = field_type.byte_size() * entry.count as usize;
        
        // If the value fits in 4 bytes, it's stored directly in value_offset
        // Otherwise, value_offset is a pointer to the actual data
        if total_bytes <= 4 {
            // Value is stored in the value_offset field itself
            let bytes = match endian {
                Endian::Little => entry.value_offset.to_le_bytes(),
                Endian::Big => entry.value_offset.to_be_bytes(),
            };
            self.parse_value_from_bytes(&bytes[..total_bytes.min(4)], field_type, entry.count, endian)
        } else {
            // Read data from the offset
            let data_start = entry.value_offset as usize;
            let data = self.read_bytes_at(data_start, total_bytes)?;
            self.parse_value_from_bytes(&data, field_type, entry.count, endian)
        }
    }

    /// Parse value from raw bytes
    fn parse_value_from_bytes(
        &self, 
        data: &[u8], 
        field_type: FieldType, 
        count: u32, 
        endian: Endian
    ) -> Result<TagValue> {
        match field_type {
            FieldType::Byte => {
                Ok(TagValue::Bytes(data.to_vec()))
            }
            FieldType::Ascii => {
                // Remove null terminator if present
                let mut string_data = data.to_vec();
                if let Some(&0) = string_data.last() {
                    string_data.pop();
                }
                let string = String::from_utf8(string_data)
                    .map_err(|_| TiffError::InvalidString { 
                        context: "ASCII tag".to_string() 
                    })?;
                Ok(TagValue::Ascii(string))
            }
            FieldType::Short => {
                let mut values = Vec::new();
                for i in 0..count as usize {
                    if i * 2 + 2 > data.len() {
                        break;
                    }
                    let bytes = [data[i * 2], data[i * 2 + 1]];
                    let value = endian.read_u16(bytes);
                    values.push(value);
                }
                Ok(TagValue::Shorts(values))
            }
            FieldType::Long => {
                let mut values = Vec::new();
                for i in 0..count as usize {
                    if i * 4 + 4 > data.len() {
                        break;
                    }
                    let bytes = [data[i * 4], data[i * 4 + 1], data[i * 4 + 2], data[i * 4 + 3]];
                    let value = endian.read_u32(bytes);
                    values.push(value);
                }
                Ok(TagValue::Longs(values))
            }
            FieldType::Rational => {
                let mut values = Vec::new();
                for i in 0..count as usize {
                    if i * 8 + 8 > data.len() {
                        break;
                    }
                    let num_bytes = [data[i * 8], data[i * 8 + 1], data[i * 8 + 2], data[i * 8 + 3]];
                    let den_bytes = [data[i * 8 + 4], data[i * 8 + 5], data[i * 8 + 6], data[i * 8 + 7]];
                    let numerator = endian.read_u32(num_bytes);
                    let denominator = endian.read_u32(den_bytes);
                    values.push((numerator, denominator));
                }
                Ok(TagValue::Rationals(values))
            }
            FieldType::SByte => {
                let values = data.iter().map(|&b| b as i8).collect();
                Ok(TagValue::SBytes(values))
            }
            FieldType::Undefined => {
                Ok(TagValue::Undefined(data.to_vec()))
            }
            FieldType::SShort => {
                let mut values = Vec::new();
                for i in 0..count as usize {
                    if i * 2 + 2 > data.len() {
                        break;
                    }
                    let bytes = [data[i * 2], data[i * 2 + 1]];
                    let value = endian.read_u16(bytes) as i16; // Convert to signed
                    values.push(value);
                }
                Ok(TagValue::SShorts(values))
            }
            FieldType::SLong => {
                let mut values = Vec::new();
                for i in 0..count as usize {
                    if i * 4 + 4 > data.len() {
                        break;
                    }
                    let bytes = [data[i * 4], data[i * 4 + 1], data[i * 4 + 2], data[i * 4 + 3]];
                    let value = endian.read_u32(bytes) as i32; // Convert to signed
                    values.push(value);
                }
                Ok(TagValue::SLongs(values))
            }
            FieldType::SRational => {
                let mut values = Vec::new();
                for i in 0..count as usize {
                    if i * 8 + 8 > data.len() {
                        break;
                    }
                    let num_bytes = [data[i * 8], data[i * 8 + 1], data[i * 8 + 2], data[i * 8 + 3]];
                    let den_bytes = [data[i * 8 + 4], data[i * 8 + 5], data[i * 8 + 6], data[i * 8 + 7]];
                    let numerator = endian.read_u32(num_bytes) as i32; // Convert to signed
                    let denominator = endian.read_u32(den_bytes) as i32; // Convert to signed
                    values.push((numerator, denominator));
                }
                Ok(TagValue::SRationals(values))
            }
            FieldType::Float => {
                let mut values = Vec::new();
                for i in 0..count as usize {
                    if i * 4 + 4 > data.len() {
                        break;
                    }
                    let bytes = [data[i * 4], data[i * 4 + 1], data[i * 4 + 2], data[i * 4 + 3]];
                    // Read as u32 first, then reinterpret as f32
                    let bits = endian.read_u32(bytes);
                    let value = f32::from_bits(bits);
                    values.push(value);
                }
                Ok(TagValue::Floats(values))
            }
            FieldType::Double => {
                let mut values = Vec::new();
                for i in 0..count as usize {
                    if i * 8 + 8 > data.len() {
                        break;
                    }
                    let bytes = [
                        data[i * 8], data[i * 8 + 1], data[i * 8 + 2], data[i * 8 + 3],
                        data[i * 8 + 4], data[i * 8 + 5], data[i * 8 + 6], data[i * 8 + 7]
                    ];
                    // Read as u64 first, then reinterpret as f64
                    let bits = endian.read_u64(bytes);
                    let value = f64::from_bits(bits);
                    values.push(value);
                }
                Ok(TagValue::Doubles(values))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_type_conversion() {
        assert_eq!(FieldType::from_u16(1).unwrap(), FieldType::Byte);
        assert_eq!(FieldType::from_u16(3).unwrap(), FieldType::Short);
        assert_eq!(FieldType::from_u16(4).unwrap(), FieldType::Long);
        assert!(FieldType::from_u16(99).is_err());
    }

    #[test]
    fn test_field_type_byte_sizes() {
        assert_eq!(FieldType::Byte.byte_size(), 1);
        assert_eq!(FieldType::Short.byte_size(), 2);
        assert_eq!(FieldType::Long.byte_size(), 4);
        assert_eq!(FieldType::Rational.byte_size(), 8);
    }

    #[test]
    fn test_tag_value_conversions() {
        let shorts = TagValue::Shorts(vec![123, 456]);
        assert_eq!(shorts.as_u32(), Some(123));
        assert_eq!(shorts.as_u16(), Some(123));

        let ascii = TagValue::Ascii("Hello".to_string());
        assert_eq!(ascii.as_string(), Some("Hello"));

        // Test signed types
        let sshorts = TagValue::SShorts(vec![-123, 456]);
        assert_eq!(sshorts.as_i32(), Some(-123));

        let slongs = TagValue::SLongs(vec![-12345, 67890]);
        assert_eq!(slongs.as_i32(), Some(-12345));

        // Test floating point (use approximate comparisons)
        let floats = TagValue::Floats(vec![3.14, 2.71]);
        assert_eq!(floats.as_f32(), Some(3.14));
        
        // For f32 -> f64 conversion, use approximate comparison
        let f64_value = floats.as_f64().unwrap();
        assert!((f64_value - 3.14).abs() < 0.001);

        let doubles = TagValue::Doubles(vec![3.14159, 2.71828]);
        assert_eq!(doubles.as_f64(), Some(3.14159));

        // Test rationals
        let rationals = TagValue::Rationals(vec![(22, 7), (355, 113)]);
        let pi_approx = rationals.as_rational_f64().unwrap();
        assert!((pi_approx - 3.142857).abs() < 0.001);

        let srationals = TagValue::SRationals(vec![(-22, 7)]);
        let neg_pi = srationals.as_rational_f64().unwrap();
        assert!((neg_pi + 3.142857).abs() < 0.001);
    }

    #[test]
    fn test_ifd_entry_creation() {
        let entry = IfdEntry {
            tag: 256,           // ImageWidth
            field_type: 4,      // Long
            count: 1,
            value_offset: 1920,
        };

        assert_eq!(entry.tag, 256);
        assert_eq!(entry.count, 1);
    }

    #[test]
    fn test_image_summary() {
        let summary = ImageSummary {
            width: 1920,
            height: 1080,
            samples_per_pixel: 3,
            bits_per_sample: vec![8, 8, 8],
            compression: Compression::None,
            photometric_interpretation: Some(PhotometricInterpretation::Rgb),
            is_tiled: false,
        };

        assert_eq!(summary.bits_per_pixel(), 24);
        assert_eq!(summary.bytes_per_pixel(), 3);
        assert!(!summary.is_grayscale());
        assert!(summary.is_rgb());
        assert!(!summary.has_alpha());

        let desc = summary.description();
        assert!(desc.contains("1920x1080"));
        assert!(desc.contains("RGB"));
        assert!(desc.contains("24-bit"));
        assert!(desc.contains("stripped"));
    }

    #[test]
    fn test_image_summary_grayscale_alpha() {
        let summary = ImageSummary {
            width: 800,
            height: 600,
            samples_per_pixel: 2,
            bits_per_sample: vec![8, 8],
            compression: Compression::Lzw,
            photometric_interpretation: Some(PhotometricInterpretation::BlackIsZero),
            is_tiled: true,
        };

        assert_eq!(summary.bits_per_pixel(), 16);
        assert_eq!(summary.bytes_per_pixel(), 2);
        assert!(summary.is_grayscale());
        assert!(!summary.is_rgb());
        assert!(summary.has_alpha());

        let desc = summary.description();
        assert!(desc.contains("Grayscale+Alpha"));
        assert!(desc.contains("tiled"));
    }

    #[test]
    fn test_image_summary_rgba() {
        let summary = ImageSummary {
            width: 512,
            height: 512,
            samples_per_pixel: 4,
            bits_per_sample: vec![8, 8, 8, 8],
            compression: Compression::PackBits,
            photometric_interpretation: Some(PhotometricInterpretation::Rgb),
            is_tiled: false,
        };

        assert_eq!(summary.bits_per_pixel(), 32);
        assert_eq!(summary.bytes_per_pixel(), 4);
        assert!(!summary.is_grayscale());
        assert!(summary.is_rgb());
        assert!(summary.has_alpha());

        let desc = summary.description();
        assert!(desc.contains("RGBA"));
        assert!(desc.contains("32-bit"));
    }

    // TODO: Add tests for actual IFD reading once we have test data
    // This will require creating mock TIFF data with a proper IFD structure
}