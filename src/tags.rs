// tiff-core/src/tags.rs
//! TIFF tag definitions and utilities
//!
//! This module defines all the standard TIFF tags and provides utilities
//! for working with tag values. Tags are the core of TIFF metadata - they
//! tell us what each piece of data in an IFD represents.

/// Standard TIFF tags
///
/// These are the official tag numbers defined in the TIFF specification.
/// Each tag represents a specific piece of metadata about the image.
pub mod tags {
    // =============================================================================
    // Basic image information
    // =============================================================================

    /// Width of the image in pixels
    pub const IMAGE_WIDTH: u16 = 256;
    /// Height of the image in pixels  
    pub const IMAGE_LENGTH: u16 = 257;
    /// Number of bits per sample (per channel)
    pub const BITS_PER_SAMPLE: u16 = 258;
    /// Compression scheme used on the image data
    pub const COMPRESSION: u16 = 259;
    /// Color space interpretation of the image data
    pub const PHOTOMETRIC_INTERPRETATION: u16 = 262;

    // =============================================================================
    // Image data organization
    // =============================================================================

    /// Offsets to strips of image data
    pub const STRIP_OFFSETS: u16 = 273;
    /// Number of samples (channels) per pixel
    pub const SAMPLES_PER_PIXEL: u16 = 277;
    /// Number of rows per strip
    pub const ROWS_PER_STRIP: u16 = 278;
    /// Byte counts for strips
    pub const STRIP_BYTE_COUNTS: u16 = 279;

    // =============================================================================
    // Resolution and units
    // =============================================================================

    /// Horizontal resolution (pixels per resolution unit)
    pub const X_RESOLUTION: u16 = 282;
    /// Vertical resolution (pixels per resolution unit)
    pub const Y_RESOLUTION: u16 = 283;
    /// Resolution unit (inches, centimeters, etc.)
    pub const RESOLUTION_UNIT: u16 = 296;

    // =============================================================================
    // Color information
    // =============================================================================

    /// Color map for palette images
    pub const COLORMAP: u16 = 320;
    /// Extra samples (alpha channel, etc.)
    pub const EXTRA_SAMPLES: u16 = 338;
    /// Sample format (unsigned, signed, float, etc.)
    pub const SAMPLE_FORMAT: u16 = 339;

    // =============================================================================
    // Tiled images (alternative to strips)
    // =============================================================================

    /// Width of tiles in pixels
    pub const TILE_WIDTH: u16 = 322;
    /// Height of tiles in pixels
    pub const TILE_LENGTH: u16 = 323;
    /// Offsets to tiles of image data
    pub const TILE_OFFSETS: u16 = 324;
    /// Byte counts for tiles
    pub const TILE_BYTE_COUNTS: u16 = 325;

    // =============================================================================
    // Compression-related
    // =============================================================================

    /// Predictor for compression (used with LZW and Deflate)
    pub const PREDICTOR: u16 = 317;

    // =============================================================================
    // Metadata
    // =============================================================================

    /// Image description/title
    pub const IMAGE_DESCRIPTION: u16 = 270;
    /// Make of scanner/camera
    pub const MAKE: u16 = 271;
    /// Model of scanner/camera  
    pub const MODEL: u16 = 272;
    /// Software used to create the image
    pub const SOFTWARE: u16 = 305;
    /// Date and time of image creation
    pub const DATE_TIME: u16 = 306;
    /// Artist/photographer
    pub const ARTIST: u16 = 315;
    /// Copyright notice
    pub const COPYRIGHT: u16 = 33432;

    // =============================================================================
    // GeoTIFF tags (we'll need these later)
    // =============================================================================

    /// Model pixel scale (for geographic images)
    pub const MODEL_PIXEL_SCALE: u16 = 33550;
    /// Model tie points (for geographic images)
    pub const MODEL_TIEPOINT: u16 = 33922;
    /// Model transformation matrix
    pub const MODEL_TRANSFORMATION: u16 = 34264;
    /// GeoKey directory
    pub const GEO_KEY_DIRECTORY: u16 = 34735;
    /// GeoKey double parameters
    pub const GEO_DOUBLE_PARAMS: u16 = 34736;
    /// GeoKey ASCII parameters
    pub const GEO_ASCII_PARAMS: u16 = 34737;
}

/// Compression types
///
/// These values appear in the Compression tag (259) and tell us
/// how the image data is compressed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Compression {
    /// No compression
    None = 1,
    /// CCITT Group 3 1-Dimensional Modified Huffman RLE
    Ccitt1d = 2,
    /// CCITT Group 3 fax encoding
    Group3Fax = 3,
    /// CCITT Group 4 fax encoding
    Group4Fax = 4,
    /// LZW compression (common for GeoTIFF)
    Lzw = 5,
    /// JPEG compression (old-style)
    JpegOld = 6,
    /// JPEG compression
    Jpeg = 7,
    /// Deflate compression (ZIP)
    Deflate = 8,
    /// Adobe Deflate
    AdobeDeflate = 32946,
    /// PackBits compression
    PackBits = 32773,
}

impl Compression {
    /// Convert from u32 to Compression
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            1 => Some(Compression::None),
            2 => Some(Compression::Ccitt1d),
            3 => Some(Compression::Group3Fax),
            4 => Some(Compression::Group4Fax),
            5 => Some(Compression::Lzw),
            6 => Some(Compression::JpegOld),
            7 => Some(Compression::Jpeg),
            8 => Some(Compression::Deflate),
            32946 => Some(Compression::AdobeDeflate),
            32773 => Some(Compression::PackBits),
            _ => None,
        }
    }

    /// Check if this compression type is supported by our parser
    pub fn is_supported(self) -> bool {
        match self {
            Compression::None => true,
            Compression::PackBits => true, // TODO: implement
            Compression::Lzw => false,     // TODO: implement
            Compression::Deflate => false, // TODO: implement
            _ => false,
        }
    }
}

/// Photometric interpretation values
///
/// These values appear in the PhotometricInterpretation tag (262)
/// and tell us how to interpret the pixel values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhotometricInterpretation {
    /// Min value is white (bilevel/grayscale)
    WhiteIsZero = 0,
    /// Min value is black (bilevel/grayscale)
    BlackIsZero = 1,
    /// RGB color model
    Rgb = 2,
    /// Palette/indexed color
    Palette = 3,
    /// Transparency mask
    TransparencyMask = 4,
    /// CMYK color model
    Cmyk = 5,
    /// YCbCr color model
    YCbCr = 6,
    /// CIE L*a*b* color model
    CieLab = 8,
}

impl PhotometricInterpretation {
    /// Convert from u32 to PhotometricInterpretation
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(PhotometricInterpretation::WhiteIsZero),
            1 => Some(PhotometricInterpretation::BlackIsZero),
            2 => Some(PhotometricInterpretation::Rgb),
            3 => Some(PhotometricInterpretation::Palette),
            4 => Some(PhotometricInterpretation::TransparencyMask),
            5 => Some(PhotometricInterpretation::Cmyk),
            6 => Some(PhotometricInterpretation::YCbCr),
            8 => Some(PhotometricInterpretation::CieLab),
            _ => None,
        }
    }
}

/// Resolution units
///
/// These values appear in the ResolutionUnit tag (296) and specify
/// the units for X/Y resolution values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResolutionUnit {
    /// No absolute unit (just relative)
    None = 1,
    /// Inch
    Inch = 2,
    /// Centimeter
    Centimeter = 3,
}

impl ResolutionUnit {
    /// Convert from u32 to ResolutionUnit
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            1 => Some(ResolutionUnit::None),
            2 => Some(ResolutionUnit::Inch),
            3 => Some(ResolutionUnit::Centimeter),
            _ => None,
        }
    }
}

/// Sample format types
///
/// These values appear in the SampleFormat tag (339) and specify
/// how to interpret the bits in each sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SampleFormat {
    /// Unsigned integer
    UInt = 1,
    /// Signed integer
    Int = 2,
    /// IEEE floating point
    Float = 3,
    /// Undefined
    Undefined = 4,
}

impl SampleFormat {
    /// Convert from u32 to SampleFormat
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            1 => Some(SampleFormat::UInt),
            2 => Some(SampleFormat::Int),
            3 => Some(SampleFormat::Float),
            4 => Some(SampleFormat::Undefined),
            _ => None,
        }
    }
}

/// Extra sample types
///
/// These values appear in the ExtraSamples tag (338) and specify
/// what additional samples beyond the basic color represent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExtraSample {
    /// Unspecified data
    Unspecified = 0,
    /// Associated alpha (premultiplied)
    AssociatedAlpha = 1,
    /// Unassociated alpha
    UnassociatedAlpha = 2,
}

impl ExtraSample {
    /// Convert from u32 to ExtraSample
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(ExtraSample::Unspecified),
            1 => Some(ExtraSample::AssociatedAlpha),
            2 => Some(ExtraSample::UnassociatedAlpha),
            _ => None,
        }
    }
}

/// Get a human-readable name for a tag
///
/// This is useful for debugging and displaying tag information.
pub fn tag_name(tag: u16) -> &'static str {
    match tag {
        tags::IMAGE_WIDTH => "ImageWidth",
        tags::IMAGE_LENGTH => "ImageLength",
        tags::BITS_PER_SAMPLE => "BitsPerSample",
        tags::COMPRESSION => "Compression",
        tags::PHOTOMETRIC_INTERPRETATION => "PhotometricInterpretation",
        tags::STRIP_OFFSETS => "StripOffsets",
        tags::SAMPLES_PER_PIXEL => "SamplesPerPixel",
        tags::ROWS_PER_STRIP => "RowsPerStrip",
        tags::STRIP_BYTE_COUNTS => "StripByteCounts",
        tags::X_RESOLUTION => "XResolution",
        tags::Y_RESOLUTION => "YResolution",
        tags::RESOLUTION_UNIT => "ResolutionUnit",
        tags::COLORMAP => "ColorMap",
        tags::TILE_WIDTH => "TileWidth",
        tags::TILE_LENGTH => "TileLength",
        tags::TILE_OFFSETS => "TileOffsets",
        tags::TILE_BYTE_COUNTS => "TileByteCounts",
        tags::PREDICTOR => "Predictor",
        tags::SAMPLE_FORMAT => "SampleFormat",
        tags::EXTRA_SAMPLES => "ExtraSamples",
        tags::IMAGE_DESCRIPTION => "ImageDescription",
        tags::MAKE => "Make",
        tags::MODEL => "Model",
        tags::SOFTWARE => "Software",
        tags::DATE_TIME => "DateTime",
        tags::ARTIST => "Artist",
        tags::COPYRIGHT => "Copyright",
        tags::MODEL_PIXEL_SCALE => "ModelPixelScale",
        tags::MODEL_TIEPOINT => "ModelTiepoint",
        tags::MODEL_TRANSFORMATION => "ModelTransformation",
        tags::GEO_KEY_DIRECTORY => "GeoKeyDirectory",
        tags::GEO_DOUBLE_PARAMS => "GeoDoubleParams",
        tags::GEO_ASCII_PARAMS => "GeoAsciiParams",
        _ => "Unknown",
    }
}

/// Check if a tag is required for basic TIFF compliance
pub fn is_required_tag(tag: u16) -> bool {
    match tag {
        tags::IMAGE_WIDTH | tags::IMAGE_LENGTH | tags::STRIP_OFFSETS | tags::STRIP_BYTE_COUNTS => {
            true
        }
        _ => false,
    }
}

/// Check if a tag contains image layout information
pub fn is_layout_tag(tag: u16) -> bool {
    match tag {
        tags::IMAGE_WIDTH
        | tags::IMAGE_LENGTH
        | tags::BITS_PER_SAMPLE
        | tags::SAMPLES_PER_PIXEL
        | tags::ROWS_PER_STRIP
        | tags::TILE_WIDTH
        | tags::TILE_LENGTH => true,
        _ => false,
    }
}

/// Check if a tag contains image data location information
pub fn is_data_location_tag(tag: u16) -> bool {
    match tag {
        tags::STRIP_OFFSETS
        | tags::STRIP_BYTE_COUNTS
        | tags::TILE_OFFSETS
        | tags::TILE_BYTE_COUNTS => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_conversion() {
        assert_eq!(Compression::from_u32(1), Some(Compression::None));
        assert_eq!(Compression::from_u32(5), Some(Compression::Lzw));
        assert_eq!(Compression::from_u32(32773), Some(Compression::PackBits));
        assert_eq!(Compression::from_u32(99999), None);
    }

    #[test]
    fn test_compression_support() {
        assert!(Compression::None.is_supported());
        assert!(Compression::PackBits.is_supported());
        assert!(!Compression::Lzw.is_supported()); // TODO: implement
        assert!(!Compression::Jpeg.is_supported());
    }

    #[test]
    fn test_photometric_interpretation() {
        assert_eq!(
            PhotometricInterpretation::from_u32(0),
            Some(PhotometricInterpretation::WhiteIsZero)
        );
        assert_eq!(
            PhotometricInterpretation::from_u32(2),
            Some(PhotometricInterpretation::Rgb)
        );
        assert_eq!(PhotometricInterpretation::from_u32(99), None);
    }

    #[test]
    fn test_tag_names() {
        assert_eq!(tag_name(tags::IMAGE_WIDTH), "ImageWidth");
        assert_eq!(tag_name(tags::COMPRESSION), "Compression");
    }

    #[test]
    fn test_tag_classification() {
        // Required tags
        assert!(is_required_tag(tags::IMAGE_WIDTH));
        assert!(is_required_tag(tags::STRIP_OFFSETS));
        assert!(!is_required_tag(tags::SOFTWARE));

        // Layout tags
        assert!(is_layout_tag(tags::IMAGE_WIDTH));
        assert!(is_layout_tag(tags::BITS_PER_SAMPLE));
        assert!(!is_layout_tag(tags::SOFTWARE));

        // Data location tags
        assert!(is_data_location_tag(tags::STRIP_OFFSETS));
        assert!(is_data_location_tag(tags::TILE_OFFSETS));
        assert!(!is_data_location_tag(tags::IMAGE_WIDTH));
    }

    #[test]
    fn test_resolution_units() {
        assert_eq!(ResolutionUnit::from_u32(2), Some(ResolutionUnit::Inch));
        assert_eq!(
            ResolutionUnit::from_u32(3),
            Some(ResolutionUnit::Centimeter)
        );
    }

    #[test]
    fn test_sample_formats() {
        assert_eq!(SampleFormat::from_u32(1), Some(SampleFormat::UInt));
        assert_eq!(SampleFormat::from_u32(3), Some(SampleFormat::Float));
    }

    #[test]
    fn test_extra_samples() {
        assert_eq!(ExtraSample::from_u32(1), Some(ExtraSample::AssociatedAlpha));
        assert_eq!(
            ExtraSample::from_u32(2),
            Some(ExtraSample::UnassociatedAlpha)
        );
    }
}
