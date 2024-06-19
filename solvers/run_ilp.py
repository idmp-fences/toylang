import argparse
from typing import List

import pulp

from aeg import AbstractEventGraph, CriticalCycle, Edge
from util import load_aeg


class ILPSolver:
    def __init__(self, aeg: AbstractEventGraph, critical_cycles: List[CriticalCycle], fences: List[Edge] = None):
        self.aeg = aeg
        self.critical_cycles = critical_cycles
        self.fences: List[Edge] = fences or []

    def fence_placement(self, time_limit=None):
        # Initialize the ILP problem
        prob = pulp.LpProblem("TSOFencePlacement", pulp.LpMinimize)

        # Set of possible fences
        possible_fences = {edge for cycle in self.critical_cycles for edge in cycle.edges}

        # Define the decision variables for each unique edge
        fences = {edge: pulp.LpVariable(f'f_{edge.id}', cat='Binary') for edge in possible_fences}

        # Objective function: Minimize the number of fences
        prob += pulp.lpSum(fences[edge] for edge in fences), "MinimizeFences"

        # Constraints: Ensure each critical cycle is broken
        for i, cycle in enumerate(self.critical_cycles):
            prob += pulp.lpSum(fences[edge] for edge in cycle.edges) >= 1, f"BreakCriticalCycle_{i}"

        # Already Placed Fences
        prob += pulp.lpSum(fences[edge] for edge in self.fences) >= len(self.fences)

        # Solver options
        solver = pulp.PULP_CBC_CMD(timeLimit=time_limit)

        # Solve the problem
        prob.solve(solver)

        # Update the aeg
        self.fences = list(edge for edge in fences if int(pulp.value(fences[edge])) == 1)
        return self.fences


def run_ilp(aeg: AbstractEventGraph, critical_cycles: List[CriticalCycle]):
    print("Starting ILP solver")
    fences = ILPSolver(aeg, critical_cycles).fence_placement()
    print("AEG:", fences)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="ILP Configuration")
    parser.add_argument("file_path", help="Path to the JSON or MSGPACK file to load")

    args = parser.parse_args()

    aeg, critical_cycles = load_aeg(args.file_path)
    run_ilp(aeg, critical_cycles)
