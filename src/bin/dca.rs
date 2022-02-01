use ca2::*;
use std::fs::File;
use std::io::{Write, Read, BufReader, BufRead, BufWriter, Error, ErrorKind, Seek, SeekFrom};
use clap::{App, Arg, ArgMatches};
use std::path::PathBuf;

// Compressed CA unpacker
//
// Command line arguments:
// 1. Input model (in ACTS or CTWedge format)
// 2. Strength
// 3. Zero or more ca2 files
struct Args {
    // Input model (ACTS/CTWedge)
    ipm: std::path::PathBuf,

    // Optional output file
    output: Option<std::path::PathBuf>,

    // Requested strength
    strength: u8,

    // Input ca2 files
    input_files: Vec<std::path::PathBuf>,

    // Disable CSV header
    no_header: bool,
}


pub fn main() -> std::io::Result<()> {
    // Grab command line arguments
    let args = parse_commandline();

    // Parse the input model
    let requested_ca = parse_request(&args.ipm, args.strength);

    // Extract CA metadata from input files
    let available_cas = parse_archives(&args.input_files)?;

    // Find the smallest compatible CA
    let best_compatible_ca = available_cas
        .iter()
        .filter(|avail| requested_ca.ca_spec.is_compatible(&avail.2))
        .fold(None, |best: Option<&(&PathBuf, u64, CASpec)>, avail| {
            if best.is_none() || avail.2.n < best.unwrap().2.n {
                Some(avail)
            } else {
                best
            }
        });

    if best_compatible_ca.is_none() {
        eprintln!("No compatible CA found in archives.");
        return Ok(());
    }

    let (file, offset, ca) = best_compatible_ca.unwrap();
    //println!("Best compatible CA: {} at offset {}: CA({}; {}, {}, {:?}",
    //         file.display(), offset, ca.n, ca.t, ca.vs.len(), ca.vs);
    let output = setup_output(&args.output)?;
    decode_ca(file, offset, ca, requested_ca, output, args.no_header)?;

    eprintln!("Decompressed CA with {} rows.", ca.n);
    return Ok(());
}

fn setup_output(output: &Option<PathBuf>) -> std::io::Result<Box<dyn Write>> {
    if output.is_some() {
        let out_f = File::create(&(output.as_ref().unwrap()))?;
        return Ok(Box::new(BufWriter::new(out_f)));
    }
    Ok(Box::new(std::io::stdout()))
}

fn decode_ca(file: &PathBuf, offset: &u64, ca_spec: &CASpec
             , requested_ca: RequestedCA, mut output: Box<dyn Write>, no_header: bool) -> std::io::Result<()> {
    let f = File::open(file)?;
    let mut reader = BufReader::new(f);
    reader.seek(SeekFrom::Start(*offset))?;

    // Create the mapping between stored and requested parameters
    let reorder_map = generate_reorder_map(&requested_ca.parameter_sizes, ca_spec);

    // Print header
    if !no_header {
        output.write((requested_ca.parameter_names.join(",") + "\n").as_bytes())?;
    }

    // Get an iterator over compressed rows
    let compressed_ca = CompressedCA::new(reader, generate_bit_sizes(&ca_spec.vs), ca_spec.n);
    for row in compressed_ca {
        // This might be a bit confusing.
        // We iterate over the reorder map (which maps stored parameters to requested parameters).
        // We take the decoded value modulo the requested parameter size
        // and use the result to grab the parameter value string,
        // thus translating it to the correct output value.
        // Finally, we join all the values in the row with a comma and append a linebreak.
        output.write((reorder_map.iter().map(
            |i| requested_ca.parameter_values[*i][
                (row[*i] % requested_ca.parameter_sizes[*i]) as usize
            ].as_str()
        ).collect::<Vec<&str>>().join(",") + "\n").as_bytes())?;
    }

    Ok(())
}

// The reorder map contains a mapping of requested parameters to stored parameters.
// reorder_map[i] points to the index of the requested parameter i in a decoded row.
fn generate_reorder_map(requested_parameter_sizes: &Vec<u16>, stored_ca_spec: &CASpec) -> Vec<usize> {
    let mut reorder_map = Vec::with_capacity(requested_parameter_sizes.len());
    for req_size in requested_parameter_sizes.iter() {
        let (mapping, _v) = stored_ca_spec
                .vs.iter().enumerate()
                .find(|(i, v)| req_size <= *v && !reorder_map.contains(i))
                .expect("Reorder mapping is broken, this should never happen");
        reorder_map.push(mapping);
    }
    reorder_map
}

fn parse_request(path: &PathBuf, strength: u8) -> RequestedCA {
    let mut file = File::open(path).expect("Unable to open the ACTS file");
    let mut contents = String::new();
    file.read_to_string(&mut contents).expect("Unable to read the ACTS file");

    // Try to parse as ACTS file
    let acts_result = try_parse_acts(&contents, strength);
    if acts_result.is_some() {
        return acts_result.unwrap();
    }
    let ctwedge_result = try_parse_ctwedge(&contents, strength);
    if ctwedge_result.is_some() {
        return ctwedge_result.unwrap();
    }
    unimplemented!("This input file format is not supported.");
}

fn parse_archives(input_files: &Vec<PathBuf>) -> std::io::Result<Vec<(&PathBuf, u64, CASpec)>> {
    let mut out = Vec::new();

    // TODO handle
    // - no input
    // - directories
    // -- also check if an argument is a file or dir...
    for f in input_files.iter() {
        let ca_specs = extract_ca_specs(&f)?;
        for (offset, ca) in ca_specs {
            out.push((f, offset, ca));
        }
    }

    Ok(out)
}

// Extract all the CA specifications in a file
fn extract_ca_specs(input_file: &PathBuf) -> std::io::Result<Vec<(u64, CASpec)>> {
    let mut out = Vec::new();
    let mut f = File::open(input_file)?;

    // Get the file size, we don't want to search before this
    // It's slightly incorrect to use i64 here, it's supposed to be u64.
    let f_size : i64 = f.seek(SeekFrom::End(0))?.try_into().unwrap();
    // Note that we are now at the *end* of the file.

    let mut reader = BufReader::new(f);
    let metadata_res = find_metadata(&mut reader, f_size);
    if metadata_res.is_err() {
        panic!("Could not find metadata in {}: {}",
               input_file.display(), metadata_res.err().unwrap());
    }

    // Read all CA specs into a buffer
    let mut ca_metadata = Vec::new();
    reader.read_to_end(&mut ca_metadata)?;

    // Parse CA specs, one by one. They are always preceded by a u64 offset.
    let mut buf_offset = 0;
    while buf_offset < ca_metadata.len() {
        let cca_offset = u64::from_be_bytes(ca_metadata[buf_offset..buf_offset+8].try_into().unwrap());
        let (ca_spec, ca_spec_len) = CASpec::unserialize(&ca_metadata[buf_offset+8..])
            .expect("Corrupted CA specification, aborting");
        buf_offset += ca_spec_len + 8;
        out.push((cca_offset, ca_spec));
    }

    Ok(out)
}

// Attempts to find the metadata block in an archive by searching for the magic bytes.
// This search is performed from the end of the file.
fn find_metadata<R: Read+Seek>(reader: &mut BufReader<R>, file_size: i64) -> std::io::Result<u64> {
    // We want to search for the beginning of our metadata
    let mut f_offset_from_end : i64 = 0;
    loop {
        if -f_offset_from_end == file_size {
            // Reached the beginning of the file, we fail
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("No CA metadata found.")
            ));
        }

        // Seek backwards by one full BufReader buffer from our previous position
        f_offset_from_end -= reader.capacity() as i64;
        // ...but not before the beginning of the file
        if -f_offset_from_end > file_size {
            f_offset_from_end = -file_size;
        }
        reader.seek(SeekFrom::End(f_offset_from_end.try_into().unwrap()))?;

        // Try to find the first byte of MAGIC_BYTES_CA2
        loop {
            let mut _buf = vec![];
            let num_bytes = reader.read_until(MAGIC_BYTES_CA2_PRE, &mut _buf)?;
            if num_bytes == 0 {
                // EOF reached...
                break;
            }
            // EOF *not* reached, so we might have found our magic bytes.
            let mut maybe_magic = [0; MAGIC_BYTES_CA2.len()];
            let line_res = reader.read_exact(&mut maybe_magic);

            if line_res.is_ok() && &maybe_magic == MAGIC_BYTES_CA2 {
                return reader.stream_position();
            }
        }
    }
}



// Set up clap argument parser and return the matches
fn get_arg_matches() -> ArgMatches {
    App::new("pca")
    //.setting(clap::AppSettings::TrailingVarArg)
    .setting(clap::AppSettings::AllowHyphenValues)
    .arg(
        Arg::new("ipm")
            .help("The input parameter model file (an ACTS or CTWedge file)")
            .long("ipm")
            .short('i')
            .required(true)
            .takes_value(true),
    )
    .arg(
        Arg::new("output")
            .help("Path to the output CSV file. If not given, the CA will be printed to stdout (if found).")
            .long("output")
            .short('o')
            .required(false)
            .takes_value(true),
    )
    .arg(
        Arg::new("no-header")
            .help("Disable CSV header")
            .long("no-header")
            .required(false)
            .takes_value(false),
    )
    .arg(
        Arg::new("strength")
            .help("The required strength of CA")
            .short('t')
            .required(true)
            .takes_value(true),
    )
    .arg(
        Arg::new("input_files")
            .help("The input .cca and .ccmeta files (in any order)")
            .required(false)
            .takes_value(true)
            .multiple_values(true)
            .allow_invalid_utf8(true),
    )
    .get_matches()
}

// Parse and validate command line arguments
fn parse_commandline() -> Args {
    let matches = get_arg_matches();

    let ipm = PathBuf::from(matches.value_of("ipm").unwrap());
    if !ipm.is_file() {
        panic!("IPM file {} does not exist.", ipm.display());
    }

    let mut output = None;
    if matches.is_present("output") {
        let output_path = PathBuf::from(matches.value_of("output").unwrap());
        if output_path.is_file() {
            panic!("Output file {} already exists.", output_path.display());
        }
        output = Some(output_path);
    }

    let input_files : Vec<PathBuf> = matches
        .values_of_os("input_files")
        .unwrap()
        .map(|x| PathBuf::from(x))
        .collect();

    for f in input_files.iter() {
        if !f.is_file() {
            panic!("CA archive {} does not exist", f.display());
        }
    }

    if input_files.len() == 0 {
        panic!("No valid input files.");
    }

    let strength = u8::from_str_radix(matches.value_of("strength").unwrap(), 10)
        .expect("Invalid strength");
    if strength < 1 {
        // To be fair, strength 1 also doesn't make much sense,
        // but at that point, it's just the user being dumb
        println!("Strength {} does not make sense.", strength);
    }

    let no_header = matches.is_present("no-header");

    Args { ipm, strength, input_files, output, no_header }
}
