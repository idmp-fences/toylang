#!/bin/bash

# Define arrays of filenames and run types
filenames=("./programs/barrier.toy" "./programs/lb.toy")
run_types=("ILP" "ALNS")

# Path to the script that needs to be run
script_path="./run_experiment.sh"

# Ensure the script is executable
chmod +x $script_path

# Function to run combinations
run_combinations() {
    for filename in "${filenames[@]}"; do
        for run_type in "${run_types[@]}"; do
            directory_name="output/${filename}_${run_type}"
            mkdir -p "$directory_name"

            output_path="${directory_name}/output.txt"

            echo "Running $script_path with $filename and $run_type"
            $script_path $filename $run_type > "$output_path"
            echo "Output saved to $output_path"
        done
    done
}

run_combinations
