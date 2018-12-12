# shift_or_euc

[![Apache 2 / MIT dual-licensed](https://img.shields.io/badge/license-Apache%202%20%2F%20MIT-blue.svg)](https://github.com/hsivonen/shift_or_euc/blob/master/COPYRIGHT)

An unoptimized and totally experimental Japanese encoding detector.

[Description of algorithm.](https://github.com/whatwg/encoding/issues/157)

## Licensing

See the file named [COPYRIGHT](https://github.com/hsivonen/shift_or_euc/blob/master/COPYRIGHT).

## Usage

1. [Install Rust](https://rustup.rs/)
2. `git clone https://github.com/hsivonen/shift_or_euc`
3. `cd shift_or_euc`
4. `cargo run --example detect PATH_TO_FILE`

The program prints one of:

* Shift_JIS
* EUC-JP
* ISO-2022-JP
