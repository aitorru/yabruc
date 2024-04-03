#!/bin/bash

# Source file
src_file="$(pwd)/test/bru-rs/Example GET.bru"

# Destination directory
dest_dir="$(pwd)/test/bru-rs/dist"

# Ensure the destination directory exists
mkdir -p $dest_dir

# Create 100 copies of the source file
for i in {1..1000}
do
    cp "$src_file" "$dest_dir/copy$i.bru"
done