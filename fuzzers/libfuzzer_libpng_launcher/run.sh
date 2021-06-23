#!/bin/sh

rm -rf ./crashes
rm -rf ./fuzzer_libpng

cargo build --release || exit 1

if [ ! -d libpng-1.6.37 ]; then
    wget https://deac-fra.dl.sourceforge.net/project/libpng/libpng16/1.6.37/libpng-1.6.37.tar.xz || exit 1
    tar -xvf libpng-1.6.37.tar.xz
fi

# Build libpng.a
if [ ! -e libpng-1.6.37/.libs/libpng16.a ]; then
    cd libpng-1.6.37
    ./configure || exit 1
    make CC=$(realpath ../target/release/libafl_cc) CXX=$(realpath ../target/release/libafl_cxx) -j `nproc` || exit 1
    cd ..
fi

# Compile the harness
./target/release/libafl_cxx ./harness.cc libpng-1.6.37/.libs/libpng16.a -I libpng-1.6.37/ -o fuzzer_libpng -lz -lm || exit 1

echo "WAS?"
timeout 3s ./fuzzer_libpng --cores 0 2>/dev/null

rm -rf ./fuzzer_libpng
rm -rf ./broker_log
exit 0