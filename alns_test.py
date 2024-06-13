import json
import sys
from copy import deepcopy
from typing import List, Set

from alns import ALNS
from alns.accept import HillClimbing
from alns.select import RandomSelect
from alns.stop import MaxRuntime

import numpy.random as rnd
import numpy as np

from ilp import AbstractEventGraph, CriticalCycle, ILPSolver

class ProblemInstance:
    def __init__(self, aeg: AbstractEventGraph, critical_cycles: List[CriticalCycle]):
        self.aeg = aeg
        self.critical_cycles = critical_cycles


class ProblemState:
    """
        fences: Dictionary with edges as keys and binary values indicating whether a fence is placed (1) or not (0).
        Note that the placement of fences may not be a valid solution to the problem instance, let alone optimal.
    """

    def __init__(self, instance: ProblemInstance):
        self.instance = instance

    def copy(self):
        # Perform a deep copy of the instance to ensure all nested objects are copied
        return ProblemState(deepcopy(self.instance))

    def objective(self) -> float:
        return len(self.instance.aeg.fences)

    def get_context(self):
        # TODO implement a method returning a context vector. This is only
        #  needed for some context-aware bandit selectors from MABWiser;
        #  if you do not use those, this default is already sufficient!
        return None


def initial_state(aeg: AbstractEventGraph, critical_cycles: List[CriticalCycle]) -> ProblemState:
    solver = ILPSolver(aeg, critical_cycles)
    solver.fence_placement(0.5)  # Run the ILP solver to place initial fences

    return ProblemState(ProblemInstance(aeg, critical_cycles))


def destroy(current: ProblemState, rnd_state: rnd.RandomState) -> ProblemState:
    # Copy the current state to avoid modifying the original state
    next_state = current.copy()

    # Randomly destroy 20% of fences (minimum 1)
    destroy_pct = 0.20
    fenced_edges = next_state.instance.aeg.fences

    num_to_destroy = max(1, int(destroy_pct * len(fenced_edges)))
    idx_destroyed = rnd_state.choice(len(fenced_edges), num_to_destroy, replace=False)

    # Remove the selected fences
    for idx in sorted(idx_destroyed, reverse=True):
        del fenced_edges[idx]

    return next_state


def repair(destroyed: ProblemState, rnd_state: rnd.RandomState) -> ProblemState:
    print(destroyed.instance.aeg)
    print(destroyed.instance.critical_cycles)
    solver = ILPSolver(destroyed.instance.aeg, destroyed.instance.critical_cycles)
    solver.fence_placement(0.5)  # Run the ILP solver to place initial fences
    print("----")
    print(destroyed.instance.aeg)
    print(destroyed.instance.critical_cycles)
    return destroyed

def greedy_repair_most_cycles(destroyed: ProblemState, rnd_state: rnd.RandomState) -> ProblemState:
    edge_cycles = {}
    id_cycle = 0
    for cycle in destroyed.instance.critical_cycles:
        for edge in cycle.edges:
            if not edge.id in edge_cycles:
                edge_cycles[edge.id] = set()
            edge_cycles[edge.id].add(id_cycle)
        id_cycle += 1

    while edge_cycles != {}:
        best_edge = -1
        cycle_count = 0
        for edge_id, cycles in edge_cycles.items():
            if len(cycles) > cycle_count:
                cycle_count = len(cycles)
                best_edge = edge_id
        if destroyed.instance.aeg.edges[best_edge].id != best_edge:
            raise Exception("Wrong json format: Edges ids are not in order")
        removed_cycles = edge_cycles[best_edge].copy()
        for cycle_id in removed_cycles:
            for edge in destroyed.instance.critical_cycles[cycle_id].edges:
                del edge_cycles[edge.id]
            
        destroyed.instance.aeg.fences.append(destroyed.instance.aeg.edges[best_edge])
    return destroyed

def greedy_repair_in_degrees(destroyed: ProblemState, rnd_state: rnd.RandomState) -> ProblemState:
    in_degrees = {}
    edge_cycles = {}
    id_cycle = 0
    for cycle in destroyed.instance.critical_cycles:
        for edge in cycle.edges:
            if not edge.id in edge_cycles:
                edge_cycles[edge.id] = set()
            edge_cycles[edge.id].add(id_cycle)
        id_cycle += 1

    for edge in destroyed.instance.aeg.edges:
        if not edge.target in in_degrees:
            in_degrees[edge.target] = 0
        in_degrees[edge.target]+=1

    while edge_cycles != {}:
        best_edge = -1
        in_degree = 0
        for edge_id, cycles in edge_cycles.items():
            source = destroyed.instance.aeg.edges[edge_id].source
            if in_degrees[source] > in_degree:
                in_degree = in_degrees[source]
                best_edge = edge_id
        if destroyed.instance.aeg.edges[best_edge].id != best_edge:
            raise Exception("Wrong json format: Edges ids are not in order")
        removed_cycles = edge_cycles[best_edge].copy()
        for cycle_id in removed_cycles:
            for edge in destroyed.instance.critical_cycles[cycle_id].edges:
                del edge_cycles[edge.id]
                
        in_degrees[destroyed.instance.aeg.edges[best_edge].source]-=1    
        destroyed.instance.aeg.fences.append(destroyed.instance.aeg.edges[best_edge])
    return destroyed


if __name__ == "__main__":
    with open('./cycles.json', 'r') as file:
        input_json = json.load(file)
    aeg_data = input_json["aeg"]
    ccs_data = input_json["critical_cycles"]

    aeg = AbstractEventGraph(aeg_data['nodes'], aeg_data['edges'])
    critical_cycles = [CriticalCycle(cc['cycle'], cc['potential_fences'], aeg) for cc in ccs_data]

    # Create the initial solution
    init_sol = initial_state(aeg, critical_cycles)
    print(f"Initial solution objective is {init_sol.objective()}.")

    # Create ALNS and add one or more destroy and repair operators
    alns = ALNS(rnd.RandomState(seed=42))
    alns.add_destroy_operator(destroy)
    alns.add_repair_operator(greedy_repair_in_degrees)
    # alns.add_repair_operator(repair)

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