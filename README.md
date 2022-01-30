# CA²: Practical Archival and Compression of Covering Arrays

This repository contains an implementation of the approach to CA compression presented in *Leithner, M. and Simos, D. E.: CA²: Practical Archival and Compression of Covering Arrays*.

It provides three executables:

* `cca` takes a CA in CSV form, the strength, and the parameter sizes and outputs a `.caa` (raw compressed CA) and `.ccmeta` (CA metadata) file.
* `pca` packages one or more pairs of `.caa` and `.ccmeta` files and optionally an unpacker executable (like `dca`) into a single `.ca2` archive.
* `dca` takes one or more `.ca2` archives and a CA specification in ACTS or CTWedge format and returns a compatible CA in CSV form, if available.

*Please note that `dca` is not yet contained in this repository; we are currently preparing it for its public release on 2022-02-01.*

For further information, feature requests and bug reports, please contact `mleithner@sba-research.org`.
