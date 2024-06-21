import argparse
import time
from typing import List, Literal

import gurobipy as gp
from gurobipy import GRB
import pulp

from aeg import AbstractEventGraph, CriticalCycle, Edge
from util import load_aeg


class ILPSolver:
    def __init__(self, aeg: AbstractEventGraph, critical_cycles: List[CriticalCycle], fences: List[Edge] = None, solver: Literal['gurobi','cbc']='gurobi'):
        self.start_time = time.perf_counter()
        self.aeg = aeg
        self.critical_cycles = critical_cycles
        self.possible_fences = {edge for cycle in self.critical_cycles for edge in cycle.edges}
        self.fences: List[Edge] = fences or []
        self.incumbent_objective_values = []
        self.solver = solver

    def capture_incumbent_solution(self, model, where):
        if where == GRB.Callback.MIPSOL:
            incumbent_value = model.cbGet(GRB.Callback.MIPSOL_OBJ)
            self.incumbent_objective_values.append(f"({incumbent_value} {time.perf_counter() - self.start_time:.3f})")

    def fence_placement(self, *args, **kwargs):
        if self.solver == 'gurobi':
            return self._solve_gurobi(*args, **kwargs)
        elif self.solver == 'cbc':
            return self._solve_cbc(*args, **kwargs)
        raise ValueError(self.solver)
    
    def _solve_cbc(self, time_limit=None, max_nodes=None, max_solutions=None, verbose=True):
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
        return self.fences, int(pulp.value(prob.objective))

    def _solve_gurobi(self, time_limit=None, max_nodes=None, max_solutions=None, verbose=True):
        # Initialize the Gurobi model
        model = gp.Model("TSOFencePlacement")

        # Define the decision variables for each unique edge
        fences = {edge: model.addVar(vtype=GRB.BINARY, name=f'f_{edge.id}') for edge in self.possible_fences}

        # Objective function: Minimize the number of fences
        model.setObjective(gp.quicksum(fences[edge] for edge in fences), GRB.MINIMIZE)

        # Constraints: Ensure each critical cycle is broken
        for i, cycle in enumerate(self.critical_cycles):
            model.addConstr(gp.quicksum(fences[edge] for edge in cycle.edges) >= 1, f"BreakCriticalCycle_{i}")

        # Already Placed Fences
        model.addConstr(gp.quicksum(fences[edge] for edge in self.fences) >= len(self.fences))

        # Solver options
        if max_solutions is not None:
            model.setParam(GRB.Param.SolutionLimit, 1)
        # if max_nodes is not None:
        #     options.append(f"MaxNodes {max_nodes}")
        if time_limit:
            model.setParam(GRB.Param.TimeLimit, time_limit)
        if not verbose:
            model.setParam(GRB.Param.OutputFlag, 0)

        # Solve the problem
        # Set callback for capturing incumbent solutions
        model.optimize(self.capture_incumbent_solution)
        
        # Update the aeg
        self.fences = [edge for edge in fences if int(fences[edge].x) == 1]
        return self.fences, int(model.ObjVal)

def run_ilp(aeg: AbstractEventGraph, critical_cycles: List[CriticalCycle], solver, verbose=False):
    if verbose:
        print("Starting ILP solver")
    ilp = ILPSolver(aeg, critical_cycles, solver=solver)
    fences, objective = ilp.fence_placement(verbose=verbose)
    end_time = time.perf_counter()
    if verbose:
        print("Fences placed", fences)

    meta = {
    "nodes": len(aeg.nodes),
    "edges": len(aeg.edges),
    "cycles": len(critical_cycles),
    "potential-fence-placements": len(ilp.possible_fences),
    "ilp-min-fences": objective,
    "ilp-solve-time": f"{end_time - ilp.start_time:.3f}",
    "incumbent-solutions": f"{" ".join(ilp.incumbent_objective_values)}"
}
    return meta

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="ILP Configuration")
    parser.add_argument("file_path", help="Path to the JSON or MSGPACK file to load")
    parser.add_argument('-q', '--quiet', action='store_true', help="Only output basic stats for benchmarking")
    parser.add_argument('-s', '--solver', choices=['gurobi', 'cbc'], default='gurobi', help='Solver backend')
    args = parser.parse_args()

    aeg, critical_cycles = load_aeg(args.file_path)
    info = run_ilp(aeg, critical_cycles, solver=args.solver, verbose=not args.quiet)
    file_name = args.file_path.rstrip('.')
    # print(f'"{file_name}": {info}')
    print(f"{file_name}," + ",".join([str(v) for v in info.values()]))
