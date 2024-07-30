#!/bin/bash

# Loop through each project directory
for project_dir in $(find . -type d -not -path "./examples/*" -name "Cargo.toml" -exec dirname {} \;); do
    # Change to the project directory
    cd "$project_dir"

    # Run cargo publish
    cargo publish

    # Change back to the original directory
    cd "$current_dir"
done