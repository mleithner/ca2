# CA²: Practical Archival and Compression of Covering Arrays

This repository contains an implementation of the approach to CA compression presented in *Leithner, M. and Simos, D. E.: CA²: Practical Archival and Compression of Covering Arrays*.

It provides three executables:

* `cca` takes a CA in CSV form, the strength, and the parameter sizes and outputs a `.caa` (raw compressed CA) and `.ccmeta` (CA metadata) file.
* `pca` packages one or more pairs of `.caa` and `.ccmeta` files and optionally an unpacker executable (like `dca`) into a single `.ca2` archive.
* `dca` takes one or more `.ca2` archives and a CA specification in ACTS or CTWedge format and returns a compatible CA in CSV form, if available.

*Please note that the CTWedge parser of `dca` is not yet contained in this repository; we are currently preparing it for its public release on 2022-02-01.*

For further information, feature requests and bug reports, please contact `mleithner@sba-research.org`.

## Setup

You need a working Rust toolchain, e.g. from [rustup](https://rustup.rs/).
Navigate into the root directory of this repository and execute

```bash
cargo build --release
```

This produces the three executables in `./target/release/`.

## Example

Suppose you have a file `/tmp/example.csv` that contains a headerless CSV, representing a CA(4096; 6, 7, 4).

``` bash
$ head -5 /tmp/example.csv 
0,0,0,0,0,0,0
1,2,2,0,3,0,0
2,3,3,0,1,0,0
3,1,1,0,2,0,0
0,1,2,2,0,1,0
```

To compress this CA, you would use `cca`:
``` bash
./target/release/cca --no-header -c /tmp/example.csv -t 6 -v 4 -v 4 -v 4 -v 4 -v 4 -v 4 -v 4
Opening /tmp/example.csv for reading
Opening /tmp/example.cca for writing raw compressed CA data
Opening /tmp/example.ccmeta for writing metadata
Successfully compressed 4096 rows, writing metadata...
```

This creates two files: A `.cca` containing the raw compressed CA data, and a `.ccmeta` that stores metadata about this CA.
In most practical circumstances, you would repeat this step for all CAs you wish to compress.

Once you have a number of compressed CAs, you can create a `.ca2` archive file using `pca`:
``` bash
./target/release/pca -o /tmp/archive.ca2 /tmp/*.cca /tmp/*.ccmeta
Parsing CA specifications...
Reordering CAs by size...
Writing compressed CAs...
Writing metadata...
Finished writing archive /tmp/archive.ca2
```

You can now store or share this `.ca2` file.

When you want to retrieve a CA that is compatible to some specification, e.g. defined using an ACTS input file, use `dca` to decompress this data:

``` bash
./target/release/dca -t 6 --ipm acts_in.txt -o /tmp/translated.csv /tmp/archive.ca2
Decompressed CA with 4096 rows.
```

This can also be used to translate CAs (in the concrete example underlying this text, we used our toolchain to translate a numeric CA to a different specification that contains string values).
