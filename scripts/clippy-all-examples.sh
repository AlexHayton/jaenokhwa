#!/bin/bash

# Change directory to the example folder
cd examples

# Loop through each project in the example folder
for project in */; do
    # Change directory to the project folder
    cd "$project"

    # Run cargo clippy
    cargo clippy

    # Change directory back to the example folder
    cd ..
done