//! Canonical content encoding and the configuration digest tree.
//!
//! Digests are taken over the *parsed* model, never over raw file bytes, so
//! reordering fields or reflowing whitespace in a RON document leaves the
//! digest unchanged while any value change moves it. Every sequence is written
//! with a length prefix, so `[[1], [2, 3]]` and `[[1, 2], [3]]` cannot collide.

/// Version of the encoding plus hash pair below.
///
/// It travels in match verification metadata: two digests are only comparable
/// when they were produced by the same algorithm version.
pub const DIGEST_ALGORITHM_VERSION: u32 = 1;

/// A fixed-width digest over canonically encoded content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentDigest(pub u64);

impl std::fmt::Display for ContentDigest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:016x}", self.0)
    }
}

/// Accumulates the canonical byte encoding of a content subject.
#[derive(Debug, Clone)]
pub struct DigestWriter {
    state: u64,
}

impl Default for DigestWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl DigestWriter {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    /// Starts an empty encoding.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: Self::OFFSET,
        }
    }

    /// Writes raw bytes in order.
    pub fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state = (self.state ^ u64::from(*byte)).wrapping_mul(Self::PRIME);
        }
    }

    /// Writes a boolean as one byte.
    pub fn bool(&mut self, value: bool) {
        self.bytes(&[u8::from(value)]);
    }

    /// Writes an unsigned value in fixed-width little-endian form.
    pub fn u8(&mut self, value: u8) {
        self.bytes(&value.to_le_bytes());
    }

    /// Writes an unsigned value in fixed-width little-endian form.
    pub fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    /// Writes an unsigned value in fixed-width little-endian form.
    pub fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    /// Writes an unsigned value in fixed-width little-endian form.
    pub fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    /// Writes a signed value in fixed-width little-endian form.
    pub fn i8(&mut self, value: i8) {
        self.bytes(&value.to_le_bytes());
    }

    /// Writes a length-prefixed string.
    pub fn str(&mut self, value: &str) {
        self.len(value.len());
        self.bytes(value.as_bytes());
    }

    /// Writes a sequence length. Call it before encoding the elements.
    pub fn len(&mut self, len: usize) {
        self.u64(len as u64);
    }

    /// Writes a length-prefixed sequence of encodable items.
    pub fn seq<T: Digestible>(&mut self, items: &[T]) {
        self.len(items.len());
        for item in items {
            item.digest_into(self);
        }
    }

    /// Finishes the encoding.
    #[must_use]
    pub const fn finish(self) -> ContentDigest {
        ContentDigest(self.state)
    }
}

/// Content that has a canonical encoding.
pub trait Digestible {
    /// Appends this value's canonical encoding to `writer`.
    fn digest_into(&self, writer: &mut DigestWriter);

    /// Digest of this value alone, as one subject of the digest tree.
    fn content_digest(&self) -> ContentDigest
    where
        Self: Sized,
    {
        let mut writer = DigestWriter::new();
        self.digest_into(&mut writer);
        writer.finish()
    }
}

impl Digestible for u16 {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.u16(*self);
    }
}

impl Digestible for u64 {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.u64(*self);
    }
}

impl Digestible for String {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.str(self);
    }
}

/// Combines ordered subject digests into the root digest.
///
/// Changing one subject moves the root while leaving every other subject
/// digest alone, which is what makes a mismatch locatable.
#[must_use]
pub fn root_digest(subjects: &[ContentDigest]) -> ContentDigest {
    let mut writer = DigestWriter::new();
    writer.len(subjects.len());
    for subject in subjects {
        writer.u64(subject.0);
    }
    writer.finish()
}
