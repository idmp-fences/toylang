import argparse
import time
from typing import List

import pulp

from aeg import AbstractEventGraph, CriticalCycle, Edge
from util import load_aeg


class ILPSolver:
    def __init__(self, aeg: AbstractEventGraph, critical_cycles: List[CriticalCycle], fences: List[Edge] = None):
        self.aeg = aeg
        self.critical_cycles = critical_cycles
        self.possible_fences = {edge for cycle in self.critical_cycles for edge in cycle.edges}
        self.fences: List[Edge] = fences or []

    def fence_placement(self, time_limit=None, max_nodes=None, max_solutions=None, verbose=True):
        # Initialize the ILP problem
        prob = pulp.LpProblem("TSOFencePlacement", pulp.LpMinimize)

        # Define the decision variables for each unique edge
        fences = {edge: pulp.LpVariable(f'f_{edge.id}', cat='Binary') for edge in self.possible_fences}

        # Objective function: Minimize the number of fences
        prob += pulp.lpSum(fences[edge] for edge in fences), "MinimizeFences"

        # Constraints: Ensure each critical cycle is broken
        for i, cycle in enumerate(self.critical_cycles):
            prob += pulp.lpSum(fences[edge] for edge in cycle.edges) >= 1, f"BreakCriticalCycle_{i}"

        # Already Placed Fences
        prob += pulp.lpSum(fences[edge] for edge in self.fences) >= len(self.fences)

        # Solver options
        options = [] 
        if max_solutions is not None:
            options.append(f"MaxSolutions {max_solutions}")
        if max_nodes is not None:
            options.append(f"MaxNodes {max_nodes}")
        solver = pulp.PULP_CBC_CMD(options=options, timeLimit=time_limit, msg=verbose)

        # Solve the problem
        prob.solve(solver)

        # Update the aeg
        self.fences = list(edge for edge in fences if int(pulp.value(fences[edge])) == 1)
        return self.fences, prob.objective


def run_ilp(aeg: AbstractEventGraph, critical_cycles: List[CriticalCycle], verbose=False):
    if verbose:
        print("Starting ILP solver")
    start_time = time.perf_counter()
    ilp = ILPSolver(aeg, critical_cycles)
    fences, objective = ilp.fence_placement(verbose=verbose)
    if verbose:
        print("Fences placed", fences)

    meta = f"""{{
    "nodes": {len(aeg.nodes)},
    "edges": {len(aeg.edges)},
    "cycles": {len(critical_cycles)},
    "potential-fence-placements": {len(ilp.possible_fences)},
    "ilp-min-fences": {int(pulp.value(objective))},
    "ilp-solve-time": {time.perf_counter() - start_time:.3f}
}}"""
    return meta

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="ILP Configuration")
    parser.add_argument("file_path", help="Path to the JSON or MSGPACK file to load")
    parser.add_argument('-q', '--quiet', action='store_true', help="Only output basic stats for benchmarking")
    args = parser.parse_args()

    aeg, critical_cycles = load_aeg(args.file_path)
    info = run_ilp(aeg, critical_cycles, verbose=not args.quiet)
    file_name = args.file_path.rstrip('.')
    print(f'"{file_name}": {info}')
