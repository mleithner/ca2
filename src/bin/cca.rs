use ca2::*;
use std::fs::File;
use std::io::{Write, BufReader, BufWriter};
use clap::Parser;
use std::collections::HashMap;
use bzip2::write::BzEncoder;
use bzip2::Compression;

// CA compression
//
// Command line arguments:
// 1. Path to a CA file (as CSV)
// 2. Strength t
// 3. v_i, exactly in the order of parameters in the CSV
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the CA file
    #[clap(parse(from_os_str), short, long = "ca")]
    ca_file: std::path::PathBuf,

    /// Strength t
    #[clap(short, long = "strength")]
    t: u8,

    /// Parameter value counts v_i
    #[clap(short, long)]
    vs: Vec<u16>,

    // Assume the CSV file has no header line
    #[clap(short, long)]
    no_header: bool,
}

pub fn main() -> std::io::Result<()> {
    let mut ca_spec = CASpec { version: CA2_DEFAULT_VERSION, n: 0, t: 0, vs: Vec::new() };
    let args = Args::parse();
    if args.t < 2 || args.vs.len() < 2 || args.vs.len() < args.t.into() {
        panic!("Invalid strength or parameter value counts.")
    }
    if !args.ca_file.as_path().is_file() {
        panic!("CA file does not exist.");
    }
    let mut output_compressed = args.ca_file.clone();
    output_compressed.set_extension("cca");
    let mut output_meta = args.ca_file.clone();
    output_meta.set_extension("ccmeta");

    // Copy arguments
    ca_spec.t = args.t;
    ca_spec.vs = args.vs.clone();

    // Create the mapping between input columns (which can be in arbitrary order)
    // and output columns (which must be sorted descending)
    ca_spec.vs.sort_by(|a, b| b.cmp(a));
    let column_map = generate_column_map(&args.vs, &ca_spec.vs).unwrap();

    // Open the CA input file
    println!("Opening {} for reading", args.ca_file.to_string_lossy());
    let f = File::open(args.ca_file)?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(!args.no_header)
        .from_reader(BufReader::new(f));

    // Open the output files
    println!("Opening {} for writing raw compressed CA data", output_compressed.to_string_lossy());
    let f_compressed = File::create(output_compressed)?;
    let mut encoder = BzEncoder::new(BufWriter::new(f_compressed), Compression::fast());
    println!("Opening {} for writing metadata", output_meta.to_string_lossy());
    let f_meta = File::create(output_meta)?;
    let mut writer_meta = BufWriter::new(f_meta);


    // We need one hashmap for each column, storing associations from CSV value to abstract (u16) value
    let mut value_maps = Vec::with_capacity(ca_spec.vs.len());
    let mut assignment_maps : Vec<u16> = vec![0; ca_spec.vs.len()];
    for _ in 0..ca_spec.vs.len() {
        value_maps.push(HashMap::new());
    }

    for result in reader.records() {
        //println!("row {}", ca_spec.n);
        let record = result?;

        // Iterate over the *output* columns (not the ones in the CSV!)
        for column in column_map.iter() {
            //println!("Grabbing column {}", column);
            let value = record.get(*column).unwrap();
            let value_out = match value_maps[*column].get(value) {
                Some(v) => *v,
                None => {
                    let new_v = assignment_maps[*column];
                    assignment_maps[*column] += 1;
                    //println!("Inserting new value {} as {} in column {}", new_v, value, column);
                    value_maps[*column].insert(value.to_string(), new_v);
                    new_v
                }
            };
            //println!("Input value {} -> output value {:#010b}", value, value_out);

            // Encode value
            encoder.write_all(&value_out.to_be_bytes())?;
        }
        ca_spec.n += 1;
    }

    // Finish encoder
    println!("Finalizing encoding...");
    encoder.try_finish()?;

    println!("Successfully compressed {} rows, writing metadata...", ca_spec.n);
    writer_meta.write_all(MAGIC_BYTES_CCA.as_bytes())?;
    writer_meta.write_all(&ca_spec.serialize())?;

    Ok(())
}

// Creates a mapping between vs_in and vs_out so that mapping[i] returns
// the index of vs_out[i] in vs_in
fn generate_column_map(vs_in: &Vec<u16>, vs_out: &Vec<u16>) -> Option<Vec<usize>> {
    let mut column_map = Vec::with_capacity(vs_out.len());
    for &v_i in vs_out.iter() {
        let mut v_i_pos = 0;
        loop {
            v_i_pos = v_i_pos + vs_in.iter().skip(v_i_pos).position(|&v_| v_i == v_)?;
            if !column_map.contains(&v_i_pos) {
                column_map.push(v_i_pos);
                break;
            } else {
                v_i_pos += 1;
            }
        }
    }
    Some(column_map)
}
