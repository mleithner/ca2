use std::io::Read;

// Parsers
extern crate pest;
extern crate pest_derive;
pub mod parsers;
pub use self::parsers::acts::try_parse_acts;
pub use self::parsers::ctwedge::try_parse_ctwedge;


// Magic bytes for a CA2 archive
pub const MAGIC_BYTES_CA2_PRE : u8 = b'_';
pub const MAGIC_BYTES_CA2 : &[u8; 16] = b"CCAA_INDEX_FILE\n";

// Magic bytes for a compressed raw CA file; note the space!
pub const MAGIC_BYTES_CCA : &str = " CCA";

// The version of the files we are processing
pub const CA2_VERSION : u16 = 1;

// Terminator for the serialized list of v_i aka vs
pub const VS_TERMINATOR : u16 = 0;

// The CA specification contains metadata required to uncompress a CA2 file
#[derive(Debug)]
pub struct CASpec {
    pub n: u64,
    pub t: u8,
    pub vs: Vec<u16>
}

// A requested CA instance, derived from an ACTS or CTWedge input file
pub struct RequestedCA {
    pub parameter_names: Vec<String>,
    pub parameter_values: Vec<Vec<String>>,
    pub parameter_sizes: Vec<u16>,
    pub ca_spec: CASpec
}

impl CASpec {
    pub fn serialize(&self) -> Vec<u8> {
        let mut out : Vec<u8> = Vec::new();

        out.extend(CA2_VERSION.to_be_bytes());
        out.extend(self.n.to_be_bytes());
        out.extend(self.t.to_be_bytes());
        for v in self.vs.iter() {
            out.extend(v.to_be_bytes());
        }
        out.extend(VS_TERMINATOR.to_be_bytes());
        out
    }

    pub fn unserialize(buf: &[u8]) -> Option<(Self, usize)> {
        let ca2_version = u16::from_be_bytes(buf[0..2].try_into().unwrap());

        if ca2_version != CA2_VERSION {
            return None;
        }

        let n = u64::from_be_bytes(buf[2..10].try_into().unwrap());
        let t = u8::from_be_bytes(buf[10..11].try_into().unwrap());

        // Parse values
        let mut vs = Vec::new();
        let mut i = 11;
        let i_max = buf.len();

        // Loop until we reach the terminator (which is not a valid count of values)
        loop {
            if i >= i_max {
                // This should never happen
                return None;
            }
            let v = u16::from_be_bytes(buf[i..i+2].try_into().unwrap());
            i += 2; // We need 2 bytes for each u16
            if v == VS_TERMINATOR {
                break;
            }
            vs.push(v);

        }

        Some((Self { n, t, vs }, i))
    }

    #[inline]
    pub fn is_compatible(&self, stored_spec: &CASpec) -> bool {
        stored_spec.t >= self.t &&
            stored_spec.vs.len() >= self.vs.len() &&
            (0..self.vs.len()).all(|i| stored_spec.vs[i] >= self.vs[i])
    }

}

// The primitive data type used to hold compressed data for bit shifts
pub type CompressionChunk = u64;
pub type Row = Vec<u16>;

// An iterator over compressed rows
pub struct CompressedCA<R: Read> {
    // Reader for our underlying data
    reader: R,
    // The stored chunk for bit shift operations
    chunk: CompressionChunk,
    // Our position in the chunk
    pos: u8,
    // Total number of rows
    rows_total: u64,
    // Current row
    row_current: u64,
    // Bit sizes for each value in the row
    bit_sizes: Vec<u8>
}

impl<R: Read> CompressedCA<R> {
    pub fn new(reader: R, bit_sizes: Vec<u8>, rows_total: u64) -> CompressedCA<R> {
        CompressedCA {
            reader,
            rows_total,
            chunk: 0,
            pos: 0,
            row_current: 0,
            bit_sizes
        }
    }
    fn fill_chunk(&mut self) -> std::io::Result<()> {
        let mut buf = [0; (CompressionChunk::BITS/8) as usize];
        self.reader.read_exact(&mut buf)?;
        self.chunk = CompressionChunk::from_be_bytes(buf);
        //println!("Filled chunk {:#066b}", self.chunk);
        Ok(())
    }
}

// Returns rows from a compressed CA
impl<R: Read> Iterator for CompressedCA<R> {
    type Item = Row;
    fn next(&mut self) -> Option<Self::Item> {
        if self.row_current < self.rows_total {
            let mut out : Row = vec![0; self.bit_sizes.len()];
            let mut value_index = 0; // Which value in self.bit_sizes we're currently handling
            let mut bits_remaining = self.bit_sizes[value_index]; // How many bits we need for this value
            loop {
                if self.pos == 0 {
                    // We're at the beginning of a chunk and must fill it
                    if self.fill_chunk().is_err() {
                        self.rows_total = 0; // XXX Hacky
                        return None;
                    }
                }
                let bits_remain_in_chunk = (CompressionChunk::BITS as u8)-self.pos;
                let bits_available = if bits_remain_in_chunk >= bits_remaining {
                    // Got enough left in chunk
                    bits_remaining
                } else {
                    // Can only take a part of the value we need
                    bits_remain_in_chunk
                };
                if bits_available > 0 {
                    //println!("{} bits remaining in chunk, {} bits required for this value, {} bits available",
                    //         bits_remain_in_chunk, bits_remaining, bits_available);
                    self.pos += bits_available;
                    //println!("Rotating by {} bits => {:#066b}", self.pos, self.chunk.rotate_left(self.pos as u32));
                    //println!("Mask:\t{:#066b}", CompressionChunk::MAX >> (CompressionChunk::BITS-bits_available as u32));
                    //println!("Rot&mask:\t{:#066b}", self.chunk.rotate_left(self.pos as u32) & (CompressionChunk::MAX >> (CompressionChunk::BITS-bits_available as u32)));
                    //println!("out[{}] before: {:#018b}", value_index, out[value_index]);
                    out[value_index] = (out[value_index] << bits_available) |
                    (
                        self.chunk.rotate_left(self.pos as u32) &
                            (CompressionChunk::MAX >> (CompressionChunk::BITS-bits_available as u32))
                    ) as u16;
                    //println!("out[{}] after: {:#018b}", value_index, out[value_index]);
                    bits_remaining -= bits_available;
                }
                // If we couldn't fill in all required bits, loop
                if bits_remaining > 0 {
                    self.pos = 0;
                    continue;
                }
                // We have all the bits for this value
                value_index += 1;
                if value_index >= self.bit_sizes.len() {
                    // Reached the last value, our row is complete
                    break;
                }
                bits_remaining = self.bit_sizes[value_index];
            }
            self.row_current += 1;
            return Some(out);
        }
        None
    }
}

pub fn generate_bit_sizes(vs_out: &Vec<u16>) -> Vec<u8> {
    let mut bit_sizes = Vec::with_capacity(vs_out.len());
    for v in vs_out {
        bit_sizes.push(std::cmp::max(1, (*v as f64).log2().ceil() as u8));
    }
    bit_sizes
}
