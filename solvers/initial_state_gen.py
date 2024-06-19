from typing import List

from alns_instance import ProblemInstance, ProblemState
from aeg import AbstractEventGraph, CriticalCycle
from run_ilp import ILPSolver


def initial_state_ilp(aeg: AbstractEventGraph, critical_cycles: List[CriticalCycle]) -> ProblemState:
    """
    Run the ILP solver to generate an initial good state
    """
    solver = ILPSolver(aeg, critical_cycles)
    solver.fence_placement(0.5)  # Run the ILP solver to place initial fences

    return ProblemState(ProblemInstance(aeg, critical_cycles))


# Place a fence on the first edge of every critical cycle
def initial_state_hot_edges(aeg: AbstractEventGraph, critical_cycles: List[CriticalCycle]) -> ProblemState:
    """
    Generate an initial state by place an edge on each cycle's hottest fence
    """
    instance = ProblemInstance(aeg, critical_cycles)
    instance.edge_cc_count
    
    unique_edges = set()
    for cc in critical_cycles:
        hottest_edge = max(cc.edges, key=lambda e: instance.edge_cc_count[e])
        unique_edges.add(hottest_edge)

    state = ProblemState(instance)
    state.fences = list(unique_edges)    

    return state


def initial_state_first_edges(aeg: AbstractEventGraph, critical_cycles: List[CriticalCycle]) -> ProblemState:
    """
    Generate an initial state by place an edge on each cycle's hottest fence
    """
    instance = ProblemInstance(aeg, critical_cycles)
    instance.edge_cc_count
    
    unique_edges = set()
    for cc in critical_cycles:
        unique_edges.add(cc.edges[0])

    state = ProblemState(instance)
    state.fences = list(unique_edges)    

    return state
