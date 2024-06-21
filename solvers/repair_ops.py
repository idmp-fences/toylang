import numpy.random as rnd

from alns_instance import ProblemState
from run_ilp import ILPSolver
from aeg import CriticalCycle


def ilp_repair_partial(state: ProblemState, rnd_state: rnd.RandomState) -> ProblemState:
    def is_broken(cycle: CriticalCycle):
        return any([edge in state.fences for edge in cycle.edges])
    unbroken_cycles = list(filter(lambda cc: not is_broken(cc),state.instance.critical_cycles))
    
    solver = ILPSolver(state.instance.aeg, unbroken_cycles)
    fences, obj = solver.fence_placement(max_solutions=1, verbose=False)  # Run the ILP solver to place initial fences
    state.fences.extend(fences)
    return state


def ilp_repair_full(state: ProblemState, rnd_state: rnd.RandomState) -> ProblemState:
    def is_broken(cycle: CriticalCycle):
        return any([edge in state.fences for edge in cycle.edges])
    unbroken_cycles = list(filter(lambda cc: not is_broken(cc),state.instance.critical_cycles))
    
    solver = ILPSolver(state.instance.aeg, unbroken_cycles) 
    fences, obj = solver.fence_placement()  # Run the ILP solver to place initial fences
    state.fences.extend(fences)
    return state

# Randomly place fences on an unbroken cycle
def repair_unbroken_cycles_randomly(state: ProblemState, rnd_state: rnd.RandomState) -> ProblemState:
    fenced_edges = state.fences

    # greedily place edges on unbroken critical cycles
    for cycle in state.instance.critical_cycles:
        potential_edges = cycle.edges
        
        # if this critical cylce is not fenced
        if sum([1 for edge in potential_edges if edge in fenced_edges ]) == 0:
            # place a fence (randomly)
            idx = rnd_state.randint(0, len(potential_edges))
            edge = potential_edges[idx]
            state.fences.append(edge)

    return state

# Repair by placing fences on the hottest edge of a cycle
# TODO: uniform distribution
def repair_hot_fences(state: ProblemState, rnd_state: rnd.RandomState) -> ProblemState:
    fenced_edges = state.fences

    # greedily place edges on unbroken critical cycles
    for cycle in state.instance.critical_cycles:
        
        # if this critical cylce is not fenced
        if sum([1 for edge in cycle.edges if edge in fenced_edges ]) == 0:
            # place a fence on the best edge
            best_edge = max(cycle.edges, key=lambda e: state.instance.edge_cc_count[e])
            state.fences.append(best_edge)

    return state

# greedy repair heuristic for continously adding fences on edges which are involved in most cycles until no cycles are left
def greedy_repair_most_cycles(state: ProblemState, rnd_state: rnd.RandomState) -> ProblemState:
    edge_cycles = {}
    id_cycle = 0
    for cycle in state.instance.critical_cycles:
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
        if state.instance.aeg.edges[best_edge].id != best_edge:
            raise Exception("Wrong json format: Edges ids are not in order")
        removed_cycles = edge_cycles[best_edge].copy()
        for cycle_id in removed_cycles:
            for edge in state.instance.critical_cycles[cycle_id].edges:
                if edge.id in edge_cycles:
                    if len(edge_cycles[edge.id]) == 1:
                        del edge_cycles[edge.id]
                    else:
                        edge_cycles[edge.id].remove(cycle_id)
            
        state.fences.append(state.instance.aeg.edges[best_edge])
    return state

# greedy repair heuristic for continously adding fences on edges which have most incoming edges until no cycles are left
def greedy_repair_in_degrees(state: ProblemState, rnd_state: rnd.RandomState) -> ProblemState:
    in_degrees = {}
    edge_cycles = {}
    id_cycle = 0
    for cycle in state.instance.critical_cycles:
        for edge in cycle.edges:
            if not edge.id in edge_cycles:
                edge_cycles[edge.id] = set()
            edge_cycles[edge.id].add(id_cycle)
        id_cycle += 1

    for edge in state.instance.aeg.edges:
        if not edge.target in in_degrees:
            in_degrees[edge.target] = 0
        in_degrees[edge.target]+=1

    while edge_cycles != {}:
        best_edge = -1
        in_degree = 0
        for edge_id, cycles in edge_cycles.items():
            source = state.instance.aeg.edges[edge_id].source
            if in_degrees[source] > in_degree:
                in_degree = in_degrees[source]
                best_edge = edge_id
        if state.instance.aeg.edges[best_edge].id != best_edge:
            raise Exception("Wrong json format: Edges ids are not in order")
        removed_cycles = edge_cycles[best_edge].copy()
        for cycle_id in removed_cycles:
            for edge in state.instance.critical_cycles[cycle_id].edges:
                if len(edge_cycles[edge.id]) == 1:
                    del edge_cycles[edge.id]
                else:
                    edge_cycles[edge.id].remove(cycle_id)
                 
        state.fences.append(state.instance.aeg.edges[best_edge])
    return state



