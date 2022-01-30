use ca2::*;
use std::fs::File;
use std::io::{Write, Read, BufReader, BufWriter, Error, ErrorKind};
use clap::{App, Arg, ArgMatches};
use std::path::PathBuf;


// Compressed CA packager
//
// Command line arguments:
// 1. Output file (required)
// 2. Prepend file (optional)
// 3. Input files (all remaining args)
struct Args {
    /// Output file
    output_file: std::path::PathBuf,

    /// Prepend file (optional)
    prepend_file: Option<std::path::PathBuf>,

    /// Input files as pairs of (ccmeta, cca)
    input_files: Vec<(std::path::PathBuf, std::path::PathBuf)>,
}


pub fn main() -> std::io::Result<()> {
    // Grab command line arguments
    let args = parse_commandline();

    // Open output file
    let f_out = File::create(&args.output_file)?;
    let mut writer = BufWriter::new(f_out);

    // Offset in the output file
    let mut offset : u64 = 0;

    // Copy the prepend file into the output
    if args.prepend_file.is_some() {
        println!("Prepending file...");
        offset += copy_file(&mut writer, &args.prepend_file.unwrap())?;
    }

    // Parse the ccmeta files
    let mut cas = Vec::with_capacity(args.input_files.len());
    println!("Parsing CA specifications...");
    for (ccmeta, cca) in args.input_files.iter() {
        //println!("Parsing CA specification from {}", ccmeta.display());
        let ca_spec = parse_ccmeta(ccmeta)?;
        cas.push((ca_spec, cca));
    }

    // Reorder CAs
    println!("Reordering CAs by size...");
    cas.sort_by(|(a, _), (b, _)| a.n.cmp(&b.n));

    // Write the compressed CA files to the output
    println!("Writing compressed CAs...");
    let mut cca_offsets : Vec<u64> = Vec::with_capacity(cas.len());
    for (_, cca) in cas.iter() {
        cca_offsets.push(offset);
        //println!("Writing compressed CA {} at offset {}", cca.display(), offset);
        offset += copy_file(&mut writer, cca)?;
    }

    // Write metadata header
    println!("Writing metadata...");
    writer.write_all(&[MAGIC_BYTES_CA2_PRE])?;
    writer.write_all(MAGIC_BYTES_CA2)?;

    // Write CA specifications
    for (i, (ca_spec, _)) in cas.iter().enumerate() {
        // First the offset...
        writer.write_all(&cca_offsets[i].to_be_bytes())?;
        // Then the CA specification
        writer.write_all(&serialize_caspec(ca_spec))?;
    }

    println!("Finished writing archive {}", args.output_file.display());
    return Ok(());
}

// Parses a ccmeta file into a CASpec
fn parse_ccmeta(input_file : &PathBuf) -> std::io::Result<CASpec> {
    let mut f = File::open(input_file)?;

    // Ingest the entire file
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;

    let buf_noprefix = buf.strip_prefix(MAGIC_BYTES_CCA.as_bytes());
    if buf_noprefix.is_none() {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("Not a valid ccmeta file: {}", input_file.display())
        ));
    }

    let ca_spec = unserialize_caspec(buf_noprefix.unwrap());

    if ca_spec.is_none() {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("Invalid CA specification in ccmeta file: {}", input_file.display())
        ));
    }

    Ok(ca_spec.unwrap().0) // We disregard the number of read bytes
}

// Simply copies the contents of `input_file` into `writer`
fn copy_file(writer : &mut BufWriter<File>, input_file : &PathBuf) -> std::io::Result<u64> {
    let f = File::open(input_file)?;
    let mut reader = BufReader::new(f);
    std::io::copy(&mut reader, writer)
}

// Set up clap argument parser and return the matches
fn get_arg_matches() -> ArgMatches {
    App::new("pca")
    //.setting(clap::AppSettings::TrailingVarArg)
    .setting(clap::AppSettings::AllowHyphenValues)
    .arg(
        Arg::new("output_file")
            .help("The output .ca2 file")
            .short('o')
            .long("output")
            .required(true)
            .takes_value(true),
    )
    .arg(
        Arg::new("prepend_file")
            .help("An optional file to place at the beginning of the output (e.g. `dca`)")
            .short('p')
            .long("pre")
            .required(false)
            .takes_value(true),
    )
    .arg(
        Arg::new("input_files")
            .help("The input .cca and .ccmeta files (in any order)")
            .required(true)
            .takes_value(true)
            .multiple_values(true),
    )
    .get_matches()
}

// Parse and validate command line arguments
fn parse_commandline() -> Args {
    let matches = get_arg_matches();
    let mut prepend_file = None;
    let mut input_files: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();

    if matches.is_present("prepend_file") {
        let prepend_file_path = PathBuf::from(matches.value_of("prepend_file").unwrap());
        if !prepend_file_path.is_file() {
            panic!("Prepend file does not exist.");
        }
        prepend_file = Some(prepend_file_path);
    }

    let output_file = PathBuf::from(matches.value_of("output_file").unwrap());
    if output_file.is_file() {
        panic!("Output file already exists");
    }

    let input_file_paths : Vec<PathBuf> = matches
        .values_of_os("input_files")
        .unwrap()
        .map(|x| PathBuf::from(x))
        .collect();

    for ccmeta in input_file_paths
        .iter()
        .filter(|x| x.extension().is_some() && x.extension().unwrap() == "ccmeta") {
            if !ccmeta.is_file() {
                panic!("Compressed CA metadata file {} does not exist", ccmeta.display());
            }
            let ccmeta_stem = ccmeta.file_stem().unwrap();
            let cca = input_file_paths
                .iter()
                .find(|x| x.extension().is_some() &&
                      x.extension().unwrap() == "cca" &&
                      x.file_stem().unwrap() == ccmeta_stem);

            if cca.is_some() {
                if !cca.unwrap().is_file() {
                    panic!("Compressed CA file {} does not exist", ccmeta.display());
                }
                input_files.push((ccmeta.to_path_buf(), cca.unwrap().to_path_buf()));
            }
        }

    if input_files.len() == 0 {
        panic!("No valid input files.");
    }

    Args { output_file, prepend_file, input_files }
}
