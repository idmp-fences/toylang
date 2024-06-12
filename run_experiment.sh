#!/bin/bash

# Check if the correct number of arguments was given
if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <filename> <mode>"
    echo "<mode> can be 'ILP' or 'ALNS'"
    exit 1
fi

# Assign arguments to variables
FILENAME=$1
MODE=$2
OUTPUT_FILE="cycles.json"

# Run the cargo command with the given filename and save the output
cargo run -p toy find-cycles "$FILENAME" > "$OUTPUT_FILE"

# Check the mode and run corresponding command
case "$MODE" in
    ILP)
        # Command for ILP
        echo "Running ILP analysis..."
        python3 experiment.py 1 "$OUTPUT_FILE"
        ;;
    ALNS)
        # Command for ALNS
        echo "Running ALNS analysis..."
        python3 experiment.py 2 "$OUTPUT_FILE"
        ;;
    *)
        echo "Invalid mode: $MODE"
        echo "Please choose 'ILP' or 'ALNS'"
        exit 1
        ;;
esac

echo "Cleaning up..."
rm "$OUTPUT_FILE"
echo "Deleted $OUTPUT_FILE"