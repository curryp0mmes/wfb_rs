# wfb_rs
Rewriting wifibroadcast in Rust

## Features

### Forward Error Correction (FEC)
This implementation includes optional Forward Error Correction using the RaptorQ fountain code. FEC helps recover lost packets in wireless transmission, improving reliability especially in environments with interference or weak signal conditions.

#### Building with FEC support
```bash
cargo build --features=fec
```

#### Building without FEC (default)
```bash
cargo build
```

#### Using FEC
When built with FEC support, both transmitter and receiver will automatically use RaptorQ encoding/decoding:

- **Transmitter**: Encodes each UDP packet into multiple FEC packets (default: 15 packets per block)
- **Receiver**: Automatically detects FEC packets and attempts to decode them back to the original data

You can disable FEC at runtime using the `--fec-enable false` flag.

## Cross compiling for the raspi:

`cross build --release`

optimize via upx, this will save about 2/3

`upx --best --lzma target/aarch64-unknown-linux-gnu/debug/tx_cli`