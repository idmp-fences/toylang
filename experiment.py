import sys
import json
import time
import csv
import math
from ilp import ILPSolver, AbstractEventGraph, CriticalCycle
from alns_test import *

def alns_experiment(filename):
    def initial_state(aeg: AbstractEventGraph, critical_cycles: List[CriticalCycle]) -> ProblemState:
        solver = ILPSolver(aeg, critical_cycles)
        solver.fence_placement(0.01)  # Run the ILP solver to place initial fences
        return ProblemState(ProblemInstance(aeg, critical_cycles))

    with open(filename, 'r') as file:
        data = json.load(file)
    aeg_data = data["aeg"]
    ccs_data = data["critical_cycles"]

    aeg = AbstractEventGraph(aeg_data['nodes'], aeg_data['edges'])
    critical_cycles = [CriticalCycle(cc['cycle'], cc['potential_fences'], aeg) for cc in ccs_data]

    # Create the initial solution
    init_sol = initial_state(aeg, critical_cycles)
    print(f"Initial solution objective is {init_sol.objective()}.")

    # Create ALNS and add one or more destroy and repair operators
    alns = ALNS(rnd.RandomState(seed=42))
    alns.add_destroy_operator(destroy)
    alns.add_repair_operator(repair)

    # Configure ALNS
    select = RandomSelect(num_destroy=1, num_repair=1)  # see alns.select for others
    accept = HillClimbing()  # see alns.accept for others
    stop = MaxRuntime(3)  # 3 seconds; see alns.stop for others

    # Run the ALNS algorithm
    start_time = time.time()
    result = alns.iterate(init_sol, select, accept, stop)
    end_time = time.time()
    # Retrieve the final solution
    best = result.best_state
    best_objective = best.objective()
    print(f"Best heuristic solution objective is {best_objective}.")
    # print("AEG:", aeg)
    print("Alns Experiment")

    elapsed_time = end_time - start_time
    print(f"ALNS experiment took {elapsed_time:.2f} seconds.")

    return elapsed_time, best_objective

def ilp_experiment(filename):
    with open(filename, 'r') as file:
        data = json.load(file)
    aeg_data = data["aeg"]
    ccs_data = data["critical_cycles"]
    aeg = AbstractEventGraph(aeg_data['nodes'], aeg_data['edges'])
    critical_cycles = [CriticalCycle(cc['cycle'], cc['potential_fences'], aeg) for cc in ccs_data]
    start_time = time.time()
    fences = ILPSolver(aeg, critical_cycles).fence_placement()
    print("ILP experiment")

    end_time = time.time()
    elapsed_time = end_time - start_time
    print(f"ILP experiment took {elapsed_time:.2f} seconds.")

    return elapsed_time, fences

def append_to_csv(csv_filename, mode, experiment_time, extra_data=None):
    inputfile = sys.argv[2]
    rows = []

    # Read the existing rows
    with open(csv_filename, mode='r', newline='') as csvfile:
        csv_reader = csv.reader(csvfile)
        rows = list(csv_reader)

    # Update the appropriate row
    print(inputfile)
    for row in rows:
        print(row[0])
        if row[0] == inputfile and row[1].upper() == mode:
            row[3] = f"{experiment_time:.2f}"
            if extra_data is not None:
                if len(row) > 4:
                    row[4] = f"{extra_data:.2f}"
                else:
                    row.append(f"{extra_data:.2f}")
            break

    # Write back the updated rows
    with open(csv_filename, mode='w', newline='') as csvfile:
        csv_writer = csv.writer(csvfile)
        csv_writer.writerows(rows)

def main():
    if len(sys.argv) < 4:
        print("Usage: python run_functions.py <mode> <inputfile> <filename> <csv_output>")
        print("<mode> can be '1' for ILP, '2' for ALNS, or 'BOTH'")
        sys.exit(1)

    try:
        mode = str(sys.argv[1]).upper()  # Convert the first argument to a string and make it uppercase
    except ValueError:
        print("Please provide a valid string.")
        sys.exit(1)

    try:
        inputfile = str(sys.argv[2])  # Convert the second argument to a string
    except ValueError:
        print("Please provide a valid inputfile.")
        sys.exit(1)

    try:
        filename = str(sys.argv[3])  # Convert the second argument to a string
    except ValueError:
        print("Please provide a valid filename.")
        sys.exit(1)

    try:
        csv_output = str(sys.argv[4])  # Convert the third argument to a string
    except ValueError:
        print("Please provide a valid CSV output filename.")
        sys.exit(1)

    # Run the appropriate experiment(s) based on the mode
    if mode == "1":
        elapsed_time, fences = ilp_experiment(filename)
        ln_fences = len(fences)
        append_to_csv(csv_output, "ILP", elapsed_time, ln_fences)
    elif mode == "2":
        elapsed_time, best_objective = alns_experiment(filename)
        append_to_csv(csv_output, "ALNS", elapsed_time, best_objective)
    elif mode == "BOTH":
        elapsed_time, fences = ilp_experiment(filename)
        ln_fences = math.log(fences)
        append_to_csv(csv_output, "ILP", elapsed_time, ln_fences)
        elapsed_time, best_objective = alns_experiment(filename)
        append_to_csv(csv_output, "ALNS", elapsed_time, best_objective)
    else:
        print(f"Invalid mode: {mode}")
        print("Please choose '1', '2', or 'BOTH'")
        sys.exit(1)

if __name__ == "__main__":
    main()
