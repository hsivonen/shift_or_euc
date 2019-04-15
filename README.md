# shift_or_euc

[![Apache 2 / MIT dual-licensed](https://img.shields.io/badge/license-Apache%202%20%2F%20MIT-blue.svg)](https://github.com/hsivonen/shift_or_euc/blob/master/COPYRIGHT)

A Japanese legacy encoding detector for detecting between Shift_JIS, EUC-JP, and, optionally, ISO-2022-JP.

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
* Undecided
