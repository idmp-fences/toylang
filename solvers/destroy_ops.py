import numpy as np
import numpy.random as rnd

from alns_instance import ProblemState

import time
def destroy_random_30(state: ProblemState, rnd_state: rnd.RandomState) -> ProblemState:
    # Copy the current state to avoid modifying the original state
    next_state = state.copy()

    # Randomly destroy 30% of fences (minimum 1)
    destroy_pct = 0.30
    fenced_edges = next_state.fences

    num_to_destroy = max(1, int(destroy_pct * len(fenced_edges)))
    idx_destroyed = rnd_state.choice(len(fenced_edges), num_to_destroy, replace=False)

    # Remove the selected fences
    for idx in sorted(idx_destroyed, reverse=True):
        del fenced_edges[idx]

    return next_state


def destroy_random_10(state: ProblemState, rnd_state: rnd.RandomState) -> ProblemState:
    # Copy the current state to avoid modifying the original state
    next_state = state.copy()

    # Randomly destroy 10% of fences (minimum 1)
    destroy_pct = 0.10
    fenced_edges = next_state.fences

    num_to_destroy = max(1, int(destroy_pct * len(fenced_edges)))
    idx_destroyed = rnd_state.choice(len(fenced_edges), num_to_destroy, replace=False)

    # Remove the selected fences
    for idx in sorted(idx_destroyed, reverse=True):
        del fenced_edges[idx]

    return next_state


# destroy heuristic that tries to remove as many fences as possible to revive a single cycle
def destroy_fences_same_cycle(state: ProblemState, rnd_state: rnd.RandomState) -> ProblemState:
    # Copy the current state to avoid modifying the original state
    next_state = state.copy()
    edge_cycles_cnt = {}
    fence_edges = set()
    for edge in next_state.fences:
        fence_edges.add(edge.id)
    cycle_id = 0
    for cycle in next_state.instance.critical_cycles:
        for edge in cycle.edges:
            if not cycle_id in edge_cycles_cnt:
                edge_cycles_cnt[cycle_id] = set()
            if edge.id in fence_edges:
                edge_cycles_cnt[cycle_id].add(edge.id)
        cycle_id += 1

    max_edges = 0
    revived_cycle = -1
    for cycle_id, edges in edge_cycles_cnt.items():
        if len(edges) > max_edges:
            max_edges = len(edges)
            revived_cycle = cycle_id

    next_state.fences = [fence for fence in next_state.fences if fence.id not in edge_cycles_cnt[revived_cycle]]

    return next_state


# destroy all fences in the cycle with the most amount of fences
def destroy_biggest_cycle(state: ProblemState, rnd_state: rnd.RandomState) -> ProblemState:
    next_state = state.copy()

    biggest_cycle = max(state.instance.critical_cycles, key=lambda c: len([1 for edge in c.edges if edge in next_state.fences]))

    for edge in biggest_cycle.edges:
        try:
            next_state.fences.remove(edge)
        except ValueError:
            pass

    return next_state


# destroy 10% of fences that are involved in the highest % of cycles
def destroy_hot_fences(state: ProblemState, rnd_state: rnd.RandomState) -> ProblemState:
    next_state = state.copy()
    destroy_pct = 0.10

    num_to_destroy = max(1, int(destroy_pct * len(next_state.fences)))

    fence_hotness = np.array(list(map(lambda edge: next_state.instance.edge_cc_count[edge], next_state.fences)))
    t = float(sum(fence_hotness))
    probabilities = fence_hotness / t

    # weigh fences by probability
    next_state.fences = list(rnd_state.choice(next_state.fences, num_to_destroy, replace=False, p=probabilities))

    return next_state


# destroy 10% of fences that are involved in the lowest  % of cycles
def destroy_cold_fences(state: ProblemState, rnd_state: rnd.RandomState) -> ProblemState:
    next_state = state.copy()
    destroy_pct = 0.10

    num_to_destroy = max(1, int(destroy_pct * len(next_state.fences)))

    fence_coldness = np.array(list(map(lambda edge: 1./next_state.instance.edge_cc_count[edge], next_state.fences)))
    t = float(sum(fence_coldness))

    probabilities = fence_coldness / t

    # weigh fences by probability
    next_state.fences = list(rnd_state.choice(next_state.fences, num_to_destroy, replace=False, p=probabilities))

    return next_state
