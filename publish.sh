cd jaenokhwa-core || exit
cargo publish
cd ../jaenokhwa-bindings-linux || exit
cargo publish
cd ../jaenokhwa-bindings-macos || exit
cargo publish
cd ../jaenokhwa-bindings-windows || exit
cargo publish
cd .. || exit
cargo publish
