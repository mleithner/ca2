// Magic bytes for a CA2 archive
pub const MAGIC_BYTES_CA2_PRE : u8 = b'_';
pub const MAGIC_BYTES_CA2 : &[u8; 16] = b"CCAA_INDEX_FILE\n";

// Magic bytes for a compressed raw CA file; note the space!
pub const MAGIC_BYTES_CCA : &str = " CCA";

// The version of the files we are processing
pub const CA2_VERSION : u16 = 1;

// The CA specification contains metadata required to uncompress a CA2 file
pub struct CASpec {
    pub n: u64,
    pub t: u8,
    pub vs: Vec<u16>
}

// Terminator for the serialized list of v_i aka vs
pub const VS_TERMINATOR : u16 = 0;

pub fn serialize_caspec(ca_spec: &CASpec) -> Vec<u8> {
    let mut out : Vec<u8> = Vec::new();

    out.extend(CA2_VERSION.to_be_bytes());
    out.extend(ca_spec.n.to_be_bytes());
    out.extend(ca_spec.t.to_be_bytes());
    for v in ca_spec.vs.iter() {
        out.extend(v.to_be_bytes());
    }
    out.extend(VS_TERMINATOR.to_be_bytes());

    out
}

pub fn unserialize_caspec(buf: &[u8]) -> Option<(CASpec, usize)> {
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

    Some((CASpec { n, t, vs }, i))
}
