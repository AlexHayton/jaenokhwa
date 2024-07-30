#!/bin/bash

project_dirs=("jaenokhwa-core" "jaenokhwa-bindings-linux" "jaenokhwa-bindings-macos" "jaenokhwa-bindings-windows")

# Loop through each project folder
for project in project_dirs; do
    # Change directory to the project folder
    cd "$project"

    # Run cargo build with default features
    cargo build

    # Run cargo build with all features
    cargo build --features serialize,output-threaded 

    # Change directory back to the example folder
    cd ..
done