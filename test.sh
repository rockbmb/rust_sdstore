#!/bin/bash

# How many test files to generate, both input and output.
num_files=3

# Size of each input file. Should be 10M+ to create scenarios with interesting delay.
file_size="30M"

for ((i=1;i<=num_files;i++)); do
    rm -f in/filein"$i";
    rm -f out/fileout"$i";
done

for ((i=1;i<=num_files;i++)); do
    head -c $file_size </dev/urandom >in/filein"$i";
done

tmuxinator start -p concurrency.yml