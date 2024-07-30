#!/bin/bash

# Create a .cargo directory if it does not exist
mkdir -p .cargo

# Create a config file with the target set to x86_64-pc-windows-msvc
echo '[build]' > .cargo/config
echo 'target = "x86_64-pc-windows-msvc"' >> .cargo/config

# Add MINGW to the PATH
echo "PATH=$PATH:C:\msys64\mingw64\bin" >> $GITHUB_ENV