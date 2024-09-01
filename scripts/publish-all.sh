#!/bin/bash

# List the directories, jaenokhwa-core etc
project_dirs=("jaenokhwa-core" "jaenokhwa-bindings-linux" "jaenokhwa-bindings-macos" "jaenokhwa-bindings-windows")

# Loop through each project directory
for project in project_dirs; do
    # Change to the project directory
    cd "$project_dir"

    # Run cargo publish
    cargo publish

    # Change back to the original directory
    cd ..
done