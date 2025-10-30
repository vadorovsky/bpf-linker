#[expect(missing_copy_implementations, reason = "not needed")]
#[derive(Debug, thiserror::Error)]
pub enum BitcodeError {
    #[error("bitcode has invalid size, expected at least 8 bytes, got {0}")]
    InvalidSize(usize),
    #[error("bitcode is not 32-bit aligned")]
    Misaligned,
    #[error("missing bitcode magic header")]
    MissingMagicHeader,
    #[error("bitcode cursor seek out of bounds")]
    CursorOutOfBounds,
    #[error("unexpected end of bitcode")]
    UnexpectedEnd,
    #[error("unsupported abbreviation encoding: {0}")]
    UnsupportedAbbreviationEncoding(u64),
    #[error("unsupported abbreviated record ID: {0}")]
    UnsupportedAbbreviatedRecordID(u64),
    #[error("mising identification string")]
    MissingIdentificationString,
}

pub(crate) fn identification_string(buffer: &[u8]) -> Result<String, BitcodeError> {
    if buffer.len() < 8 {
        return Err(BitcodeError::InvalidSize(buffer.len()));
    }
    if buffer.len() % 4 != 0 {
        return Err(BitcodeError::Misaligned);
    }

    let mut words = Vec::with_capacity(buffer.len() / 4);
    for chunk in buffer.chunks_exact(4) {
        words.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }

    const BITCODE_MAGIC: u32 = 0xdec0_4342;
    if words.first().copied() != Some(BITCODE_MAGIC) {
        return Err(BitcodeError::MissingMagicHeader);
    }

    let mut cursor = BitCursor::new(&words);
    cursor.seek_to_bit(32)?;

    let mut blocks = vec![BlockState::root()];

    while let Some(state) = blocks.last().copied() {
        if cursor.is_eof() {
            break;
        }

        let abbrev_id = cursor.read_bits(state.code_size)?;
        match abbrev_id {
            ABBREV_ID_END_BLOCK => {
                cursor.align32()?;
                let _ = blocks.pop();
                if blocks.is_empty() {
                    break;
                }
            }
            ABBREV_ID_ENTER_SUBBLOCK => {
                let block_id = cursor.read_vbr(SUBBLOCK_ID_VBR_WIDTH)? as u32;
                let new_code_size = cursor.read_vbr(SUBBLOCK_CODE_SIZE_VBR_WIDTH)? as u32;
                cursor.align32()?;
                let _len_in_words = cursor.read_bits(32)?;
                blocks.push(BlockState::new(block_id, new_code_size));
            }
            ABBREV_ID_DEFINE_ABBREV => skip_define_abbrev(&mut cursor)?,
            ABBREV_ID_UNABBREV_RECORD => {
                let record = read_unabbrev_record(&mut cursor)?;
                if state
                    .block_id
                    .is_some_and(|id| id == IDENTIFICATION_BLOCK_ID)
                    && record.code == IDENTIFICATION_CODE_STRING
                {
                    let bytes = record
                        .operands
                        .into_iter()
                        .map(|op| op as u8)
                        .collect::<Vec<_>>();
                    let string = String::from_utf8_lossy(&bytes).into_owned();
                    return Ok(string);
                }
            }
            other => {
                return Err(BitcodeError::UnsupportedAbbreviatedRecordID(other));
            }
        }
    }

    Err(BitcodeError::MissingIdentificationString)
}

const ABBREV_ID_END_BLOCK: u64 = 0;
const ABBREV_ID_ENTER_SUBBLOCK: u64 = 1;
const ABBREV_ID_DEFINE_ABBREV: u64 = 2;
const ABBREV_ID_UNABBREV_RECORD: u64 = 3;

const IDENTIFICATION_BLOCK_ID: u32 = 13;
const IDENTIFICATION_CODE_STRING: u32 = 1;

/// VBR width used when decoding block IDs inside `ENTER_SUBBLOCK` records.
const SUBBLOCK_ID_VBR_WIDTH: u32 = 8;
/// VBR width that encodes a subblock's local abbreviation bit width.
const SUBBLOCK_CODE_SIZE_VBR_WIDTH: u32 = 4;
/// VBR width for unabbreviated record codes.
const RECORD_CODE_VBR_WIDTH: u32 = 6;
/// VBR width for the number of operands in unabbreviated records.
const RECORD_NUM_OPERANDS_VBR_WIDTH: u32 = 6;
/// VBR width for each operand within an unabbreviated record.
const RECORD_OPERAND_VBR_WIDTH: u32 = 6;
/// VBR width that encodes how many ops a `DEFINE_ABBREV` entry has.
const ABBREV_NUM_OPERANDS_VBR_WIDTH: u32 = 5;
/// VBR width for literal values inside `DEFINE_ABBREV`.
const LITERAL_VBR_WIDTH: u32 = 8;
/// VBR width for data attached to certain abbrev encodings (`Array`/`Char6`).
const ABBREV_ENCODING_DATA_VBR_WIDTH: u32 = 5;

#[derive(Clone, Copy)]
struct BlockState {
    block_id: Option<u32>,
    code_size: u32,
}

impl BlockState {
    fn root() -> Self {
        Self {
            block_id: None,
            code_size: 2,
        }
    }

    fn new(block_id: u32, code_size: u32) -> Self {
        Self {
            block_id: Some(block_id),
            code_size,
        }
    }
}

/// Bit-level reader over 32-bit word slices.
/// Tracks the current bit offset and supports arbitrary-width bitcode fields.
struct BitCursor<'a> {
    words: &'a [u32],
    bit_len: usize,
    bit_pos: usize,
}

impl<'a> BitCursor<'a> {
    fn new(words: &'a [u32]) -> Self {
        Self {
            words,
            bit_len: words.len() * 32,
            bit_pos: 0,
        }
    }

    fn seek_to_bit(&mut self, bit: usize) -> Result<(), BitcodeError> {
        if bit > self.bit_len {
            return Err(BitcodeError::CursorOutOfBounds);
        }
        self.bit_pos = bit;
        Ok(())
    }

    fn is_eof(&self) -> bool {
        self.bit_pos >= self.bit_len
    }

    /// Reads `n` bits from the current position, stitching across word
    /// boundaries when needed, and advances the cursor by that many bits.
    fn read_bits(&mut self, n: u32) -> Result<u64, BitcodeError> {
        if n == 0 {
            return Ok(0);
        }
        if self.bit_pos + n as usize > self.bit_len {
            return Err(BitcodeError::UnexpectedEnd);
        }

        let mut result = 0u64;
        let mut read = 0u32;

        while read < n {
            let word_index = self.bit_pos >> 5;
            let bit_index = self.bit_pos & 31;
            let bits_available = 32 - bit_index;
            let take = std::cmp::min(bits_available as u32, n - read);
            let mask = if take == 32 {
                u64::MAX
            } else {
                (1u64 << take) - 1
            };
            let chunk = ((self.words[word_index] as u64) >> bit_index) & mask;
            result |= chunk << read;
            self.bit_pos += take as usize;
            read += take;
        }

        Ok(result)
    }

    /// Reads an LLVM variable-bit-rate (VBR) integer.
    /// Each `width`-bit chunk uses the MSB as a continuation flag, with the
    /// remaining bits appended LSB-first until a chunk clears the flag.
    fn read_vbr(&mut self, width: u32) -> Result<u64, BitcodeError> {
        let mut result = 0u64;
        let mut shift = 0u32;
        loop {
            let piece = self.read_bits(width)?;
            let continue_bit = 1u64 << (width - 1);
            let value = piece & (continue_bit - 1);
            result |= value << shift;
            if piece & continue_bit == 0 {
                break;
            }
            shift += width - 1;
        }
        Ok(result)
    }

    /// Skips padding so the cursor advances to the next 32-bit boundary.
    /// LLVM blocks require subsequent contents to start on word-aligned offsets.
    fn align32(&mut self) -> Result<(), BitcodeError> {
        let remainder = self.bit_pos & 31;
        if remainder != 0 {
            let to_skip = 32 - remainder;
            let _ = self.read_bits(to_skip as u32)?;
        }
        Ok(())
    }
}

/// Unabbreviated LLVM.ident record containing the opcode and operands payload.
struct Record {
    code: u32,
    operands: Vec<u64>,
}

fn read_unabbrev_record(cursor: &mut BitCursor<'_>) -> Result<Record, BitcodeError> {
    let code = cursor.read_vbr(RECORD_CODE_VBR_WIDTH)? as u32;
    let num_ops = cursor.read_vbr(RECORD_NUM_OPERANDS_VBR_WIDTH)? as usize;
    let mut operands = Vec::with_capacity(num_ops);
    for _ in 0..num_ops {
        operands.push(cursor.read_vbr(RECORD_OPERAND_VBR_WIDTH)?);
    }
    Ok(Record { code, operands })
}

fn skip_define_abbrev(cursor: &mut BitCursor<'_>) -> Result<(), BitcodeError> {
    let num_ops = cursor.read_vbr(ABBREV_NUM_OPERANDS_VBR_WIDTH)? as usize;
    for _ in 0..num_ops {
        let is_literal = cursor.read_bits(1)? != 0;
        if is_literal {
            let _literal = cursor.read_vbr(LITERAL_VBR_WIDTH)?;
        } else {
            let encoding = cursor.read_bits(3)?;
            match encoding {
                1 | 2 => {
                    let _ = cursor.read_vbr(ABBREV_ENCODING_DATA_VBR_WIDTH)?;
                }
                3 | 4 | 5 => {}
                other => {
                    return Err(BitcodeError::UnsupportedAbbreviationEncoding(other));
                }
            }
        }
    }
    Ok(())
}
