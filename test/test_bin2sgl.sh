#!/bin/bash

gcc -Wall -O2 -static -s -o bin2sgl bin2sgl.c

echo -e "Getting some random data.."
head -c 1024 /dev/urandom > random.bin

echo -e "Making radio_tool binary"
../build/radio_tool --wrap -s 0x00:random.bin -r GD77 -o random_0.bin

echo -e "Making bin2sgl binary"
./bin2sgl -f random.bin -m GD-77

echo ==============
sha256sum random.sgl
sha256sum random_0.bin

echo ==============
xxd -l 128 random.sgl
echo ==============
xxd -l 128 random_0.bin

echo ==============
xxd -s -128 random.sgl
echo ==============
xxd -s -128 random_0.bin

du -b random.sgl
du -b random_0.bin

diff -y <(xxd random.sgl) <(xxd random_0.bin) > random_diff