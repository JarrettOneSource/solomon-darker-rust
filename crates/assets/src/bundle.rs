use std::fmt;

/// One sprite in a stock bundle: an atlas crop plus the logical box it registers into.
#[derive(Clone, Debug, PartialEq)]
pub struct SpriteRecord {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub logical_width: i32,
    pub logical_height: u32,
    pub center_x: f32,
    pub center_y: f32,
    /// Attachment points relative to the logical centre (for example staff sockets).
    pub points: Vec<[f32; 2]>,
}

impl SpriteRecord {
    /// Where the atlas crop's top-left corner sits inside the logical box.
    ///
    /// Native registration centres the crop in the logical box and then applies the record's
    /// centre offset. The result is kept as a float so sprites can be placed sub-pixel exactly.
    #[must_use]
    pub fn registered_origin(&self) -> [f32; 2] {
        [
            (self.logical_width as f32 - self.width as f32) / 2.0 + self.center_x,
            (self.logical_height as f32 - self.height as f32) / 2.0 + self.center_y,
        ]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum BundleError {
    Truncated { index: usize, offset: usize },
    Rotated { index: usize },
    ContentBoundsMismatch { index: usize },
    NonIntegral { index: usize, field: &'static str, value: f32 },
}

impl fmt::Display for BundleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { index, offset } => {
                write!(f, "bundle record {index} at byte {offset} runs past the end of the data")
            }
            Self::Rotated { index } => {
                write!(f, "bundle record {index} is rotated, which stock data never is")
            }
            Self::ContentBoundsMismatch { index } => {
                write!(f, "bundle record {index} has content bounds that differ from its crop")
            }
            Self::NonIntegral { index, field, value } => {
                write!(f, "bundle record {index} has a non-integral {field}: {value}")
            }
        }
    }
}

impl std::error::Error for BundleError {}

const RECORD_HEAD_BYTES: usize = 45;

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl Cursor<'_> {
    fn take<const N: usize>(&mut self, index: usize) -> Result<[u8; N], BundleError> {
        let end = self.offset + N;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(BundleError::Truncated { index, offset: self.offset })?;
        self.offset = end;
        Ok(slice.try_into().expect("slice length equals N"))
    }

    fn f32(&mut self, index: usize) -> Result<f32, BundleError> {
        Ok(f32::from_le_bytes(self.take(index)?))
    }

    fn u32(&mut self, index: usize) -> Result<u32, BundleError> {
        Ok(u32::from_le_bytes(self.take(index)?))
    }

    fn i32(&mut self, index: usize) -> Result<i32, BundleError> {
        Ok(i32::from_le_bytes(self.take(index)?))
    }

    fn u8(&mut self, index: usize) -> Result<u8, BundleError> {
        Ok(self.take::<1>(index)?[0])
    }
}

fn integral(value: f32, index: usize, field: &'static str) -> Result<u32, BundleError> {
    if value.fract() != 0.0 || value < 0.0 {
        return Err(BundleError::NonIntegral { index, field, value });
    }
    Ok(value as u32)
}

/// Parses every record in a stock `.bundle` file.
pub fn parse_bundle(bytes: &[u8]) -> Result<Vec<SpriteRecord>, BundleError> {
    let mut cursor = Cursor { bytes, offset: 0 };
    let mut records = Vec::with_capacity(bytes.len() / (RECORD_HEAD_BYTES + 8));
    while cursor.offset < bytes.len() {
        let index = records.len();
        let x = cursor.f32(index)?;
        let y = cursor.f32(index)?;
        let width = cursor.f32(index)?;
        let height = cursor.f32(index)?;
        let logical_width = cursor.i32(index)?;
        let logical_height = cursor.u32(index)?;
        let content_width = cursor.f32(index)?;
        let content_height = cursor.f32(index)?;
        let center_x = cursor.f32(index)?;
        let center_y = cursor.f32(index)?;
        let rotated = cursor.u8(index)?;
        let point_count = cursor.u32(index)? as usize;
        // Validate the declared points before allocation. A malformed/trailing font
        // record can declare more than a WASM allocation can address.
        let available_floats = (bytes.len() - cursor.offset) / 4;
        if point_count > available_floats / 2 {
            return Err(BundleError::Truncated {
                index,
                offset: cursor.offset + available_floats * 4,
            });
        }
        let mut points = Vec::with_capacity(point_count);
        for _ in 0..point_count {
            points.push([cursor.f32(index)?, cursor.f32(index)?]);
        }
        if rotated != 0 {
            return Err(BundleError::Rotated { index });
        }
        if content_width != width || content_height != height {
            return Err(BundleError::ContentBoundsMismatch { index });
        }
        records.push(SpriteRecord {
            x: integral(x, index, "atlas x")?,
            y: integral(y, index, "atlas y")?,
            width: integral(width, index, "atlas width")?,
            height: integral(height, index, "atlas height")?,
            logical_width,
            logical_height,
            center_x,
            center_y,
            points,
        });
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_bytes(points: &[[f32; 2]]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for value in [10.0f32, 20.0, 30.0, 40.0] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&64i32.to_le_bytes());
        bytes.extend_from_slice(&48u32.to_le_bytes());
        for value in [30.0f32, 40.0, 1.5, -2.0] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.push(0);
        bytes.extend_from_slice(&(points.len() as u32).to_le_bytes());
        for point in points {
            bytes.extend_from_slice(&point[0].to_le_bytes());
            bytes.extend_from_slice(&point[1].to_le_bytes());
        }
        bytes
    }

    #[test]
    fn parses_consecutive_records_with_points() {
        let mut bytes = record_bytes(&[[1.0, 2.0], [3.0, 4.0]]);
        bytes.extend(record_bytes(&[]));
        let records = parse_bundle(&bytes).expect("valid bundle");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].points, vec![[1.0, 2.0], [3.0, 4.0]]);
        assert_eq!(records[1].points, Vec::<[f32; 2]>::new());
        assert_eq!(
            records[0].registered_origin(),
            [(64.0 - 30.0) / 2.0 + 1.5, (48.0 - 40.0) / 2.0 - 2.0]
        );
    }

    #[test]
    fn truncated_data_is_an_error_with_the_record_index() {
        let mut bytes = record_bytes(&[]);
        bytes.extend(&record_bytes(&[[1.0, 2.0]])[..50]);
        assert_eq!(
            parse_bundle(&bytes),
            Err(BundleError::Truncated { index: 1, offset: 45 + 45 + 4 })
        );
    }

    #[test]
    fn oversized_point_count_is_rejected_before_allocation() {
        let mut bytes = record_bytes(&[]);
        bytes[41..45].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(parse_bundle(&bytes), Err(BundleError::Truncated { index: 0, offset: 45 }));
    }

    #[test]
    fn rotated_records_are_rejected() {
        let mut bytes = record_bytes(&[]);
        bytes[0x28] = 1;
        assert_eq!(parse_bundle(&bytes), Err(BundleError::Rotated { index: 0 }));
    }
}
