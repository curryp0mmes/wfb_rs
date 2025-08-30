# wfb_rs
Rewriting wifibroadcast in Rust


# cross compiling for the raspi:

`cross build --release`

optimize via upx, this will save about 2/3

`upx --best --lzma target/aarch64-unknown-linux-gnu/debug/tx_cli`