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

# Argument passed to bash script dictates what test to run
if [ "$1" == "conc" ]; then
    tmuxinator start -p concurrency.yml
elif [ "$1" == "prio" ]; then
    tmuxinator start -p priority.yml;
elif [ "$1" == "status" ]; then
    tmuxinator start -p status.yml;
elif [ "$1" == "impossible" ]; then
    tmuxinator start -p impossible.yml;
elif [ "$1" == "ctrl_c" ]; then
    tmuxinator start -p ctrl_c.yml;
else
    echo "Unknown test parameter! Please rerun with an appropriate argument."
fi