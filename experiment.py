import sys
import json
from ilp import ILPSolver, AbstractEventGraph, CriticalCycle
from alns_test import *

def alns_experiment(filename):
    def initial_state(aeg: AbstractEventGraph, critical_cycles: List[CriticalCycle]) -> ProblemState:
        solver = ILPSolver(aeg, critical_cycles)
        solver.fence_placement(0.5)  # Run the ILP solver to place initial fences

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
    result = alns.iterate(init_sol, select, accept, stop)

    # Retrieve the final solution
    best = result.best_state
    print(f"Best heuristic solution objective is {best.objective()}.")
    print("AEG:", aeg)
    print("Alns Experiment")

def ilp_experiment(filename):
    with open(filename, 'r') as file:
        data = json.load(file)
    aeg_data = data["aeg"]
    ccs_data = data["critical_cycles"]
    aeg = AbstractEventGraph(aeg_data['nodes'], aeg_data['edges'])
    critical_cycles = [CriticalCycle(cc['cycle'], cc['potential_fences'], aeg) for cc in ccs_data]
    print(ILPSolver(aeg, critical_cycles).fence_placement())
    print("ILP experiment")


def main():
    if len(sys.argv) < 3:
        print("Usage: python run_functions.py <number> <filename>")
        sys.exit(1)

    try:
        arg = int(sys.argv[1])  # Convert the first argument to an integer
    except ValueError:
        print("Please provide a valid integer.")
        sys.exit(1)

    try:
        filename = str(sys.argv[2])  # Convert the first argument to an integer
    except ValueError:
        print("Please provide a valid string.")
        sys.exit(1)


    # Match the argument to a function
    if arg == 1:
        ilp_experiment(filename)
    elif arg == 2:
        alns_experiment(filename)
    else:
        print(f"No function associated with the number: {arg}")

if __name__ == "__main__":
    main()

