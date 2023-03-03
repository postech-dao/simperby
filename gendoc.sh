#!/bin/bash

# This script generates the runtime documentation for the project.

# simperby-cli (docs/cli_help.txt)
cargo run -q --example help 2> docs/cli_help.txt
echo "Generated docs/cli_help.txt"
