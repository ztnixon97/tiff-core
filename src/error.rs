// tiff-core/src/error.rs
//! Error types for TIFF operations

/// TIFF-specific error type
#[derive(Debug)]
pub enum TiffError {
    /// File or data is too small to contain required structure
    InsufficientData {
        /// What we were trying to read
        operation: &'static str,
        /// How many bytes we needed
        needed: usize,
        /// How many bytes were available
        available: usize,
    },
    
    /// Invalid TIFF magic number in header
    InvalidMagic {
        /// The magic number we found
        found: u16,
    },
    
    /// Invalid byte order indicator in header
    InvalidByteOrder {
        /// The byte order bytes we found
        found: [u8; 2],
    },
    
    /// Attempted to read past the end of file
    OutOfBounds {
        /// The index we tried to access
        index: usize,
        /// The maximum valid index
        max: usize,
    },
    
    /// Invalid or unsupported field type in IFD entry
    InvalidFieldType {
        /// The field type value we found
        found: u16,
    },
    
    /// Unsupported TIFF feature
    UnsupportedFeature {
        /// Description of the unsupported feature
        feature: String,
    },
    
    /// File structure is malformed
    MalformedFile {
        /// Description of what's wrong
        reason: String,
    },
    
    /// Invalid tag data
    InvalidTag {
        /// The tag number
        tag: u16,
        /// What's wrong with it
        reason: String,
    },
    
    /// Invalid string data (e.g., non-UTF8 in ASCII tag)
    InvalidString {
        /// Context about where the invalid string was found
        context: String,
    },
}

impl std::fmt::Display for TiffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TiffError::InsufficientData { operation, needed, available } => {
                write!(f, "Insufficient data for {operation}: needed {needed} bytes, but only {available} available")
            }
            TiffError::InvalidMagic { found } => {
                write!(f, "Invalid TIFF magic number: expected 42, found {found}")
            }
            TiffError::InvalidByteOrder { found } => {
                write!(f, "Invalid byte order indicator: expected 'II' or 'MM', found {found:?}")
            }
            TiffError::OutOfBounds { index, max } => {
                write!(f, "Index {index} out of bounds (maximum: {max})")
            }
            TiffError::InvalidFieldType { found } => {
                write!(f, "Invalid field type: {found}")
            }
            TiffError::UnsupportedFeature { feature } => {
                write!(f, "Unsupported TIFF feature: {feature}")
            }
            TiffError::MalformedFile { reason } => {
                write!(f, "Malformed TIFF file: {reason}")
            }
            TiffError::InvalidTag { tag, reason } => {
                write!(f, "Invalid tag {tag}: {reason}")
            }
            TiffError::InvalidString { context } => {
                write!(f, "Invalid string data in {context}")
            }
        }
    }
}

impl std::error::Error for TiffError {}

/// Result type for TIFF operations
/// 
/// This is a convenience alias that saves you from writing 
/// `Result<T, TiffError>` everywhere
pub type Result<T> = std::result::Result<T, TiffError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let error = TiffError::InvalidMagic { found: 31 };
        assert_eq!(
            error.to_string(),
            "Invalid TIFF magic number: expected 42, found 31"
        );
    }

    #[test]
    fn test_insufficient_data_error() {
        let error = TiffError::InsufficientData {
            operation: "reading header",
            needed: 8,
            available: 4,
        };
        assert_eq!(
            error.to_string(),
            "Insufficient data for reading header: needed 8 bytes, but only 4 available"
        );
    }

    #[test]
    fn test_out_of_bounds_error() {
        let error = TiffError::OutOfBounds {
            index: 100,
            max: 50,
        };
        assert_eq!(
            error.to_string(),
            "Index 100 out of bounds (maximum: 50)"
        );
    }
}