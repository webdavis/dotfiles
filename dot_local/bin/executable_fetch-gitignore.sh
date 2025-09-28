#!/usr/bin/env bash

# Exit immediately if command fails.
set -e

file="$1"

# Fetch gitignore file.
curl -o .gitignore "https://raw.githubusercontent.com/github/gitignore/master/${file}.gitignore"
