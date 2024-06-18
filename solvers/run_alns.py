import argparse
import time
from typing import Literal, Optional

import matplotlib.pyplot as plt
import numpy as np
from alns import ALNS
from alns.accept import HillClimbing, SimulatedAnnealing
from alns.select import RandomSelect, RouletteWheel
from alns.stop import MaxRuntime

from destroy_ops import *
from initial_state_gen import *
from repair_ops import *
from aeg import AbstractEventGraph, CriticalCycle
from util import load_aeg


class UntilObjective:
    def __init__(self, min_objective: float, max_runtime: Optional[float] = None):
        if max_runtime is not None and max_runtime < 0:
            raise ValueError("max_runtime < 0 not understood.")

        self.max_runtime = max_runtime
        self.min_objective = min_objective
        self._start_runtime: Optional[float] = None

    def __call__(self, rnd, best, current) -> bool:
        if self._start_runtime is None:
            self._start_runtime = time.perf_counter()

        if current.objective() <= self.min_objective:
            return True

        if self.max_runtime is None:
            return False

        return time.perf_counter() - self._start_runtime > self.max_runtime


def run_alns(
        aeg: AbstractEventGraph,
        critical_cycles: List[CriticalCycle],
        initial_state_gen: Literal["hot-edges", "first-edges", "ilp"] = "hot-edges",
        select: Literal["random", "roulette-wheel", "roulette-wheel-segmented"] = "random",
        accept: Literal["hill-climbing", "late-acceptance-hill-climbing", "simulated-annealing"] = "hill-climbing",
        max_runtime: int = 60,
        until_objective: int = None
):
    load_time = time.perf_counter()

    # Create the initial solution
    initial_state_gen = {
        "hot-edges": initial_state_hot_edges,
        "first-edges": initial_state_first_edges,
        "ilp": initial_state_ilp
    }[initial_state_gen]

    init_sol = initial_state_gen(aeg, critical_cycles)

    init_time = time.perf_counter()

    print(f"Initial solution objective is {init_sol.objective()} ({init_time - load_time})")

    # Create ALNS and add one or more destroy and repair operators
    alns = ALNS(rnd.RandomState(seed=42))

    destroy_ops = [destroy_cold_fences, destroy_fences_same_cycle, destroy_hot_fences, destroy_random_10, destroy_random_30, destroy_biggest_cycle]
    repair_ops = [repair_unbroken_cycles_randomly, repair_hot_fences]#, greedy_repair_in_degrees, greedy_repair_most_cycles]

    for destroy in destroy_ops:
        alns.add_destroy_operator(destroy)
    for repair in repair_ops:
        alns.add_repair_operator(repair)

    # Configure ALNS
    select = {
        "random": RandomSelect(num_destroy=len(destroy_ops), num_repair=len(repair_ops)),
        "roulette-wheel": RouletteWheel([3, 2, 1, 0.5], 0.8, num_destroy=len(destroy_ops), num_repair=len(repair_ops)),
        # "roulette-wheel-segmented": RouletteWheelSegmented
    }[select]

    accept = {
        "hill-climbing": HillClimbing(),
        # "late-acceptance-hill-climbing": LateAcceptanceHillClimbing,
        "simulated-annealing": SimulatedAnnealing(start_temperature=500, end_temperature=1, step=0.95)
    }[accept]

    if until_objective is not None:
        stop = UntilObjective(until_objective, max_runtime)
    else:
        stop = MaxRuntime(max_runtime)

    # Run the ALNS algorithm
    alns.on_best(lambda state, rnd_state, **kwargs: print(f"New best objective: {state.objective()} ({time.perf_counter() - load_time})"))
    result = alns.iterate(init_sol, select, accept, stop)

    # Retrieve the final solution
    best = result.best_state
    best_iter = np.where(result.statistics.objectives == best.objective())[0][0] + 1
    print(f"Best heuristic solution objective found is {best.objective()}, found at iteration {best_iter}")

    # Plot operator & objective info
    figure = plt.figure("operator_counts", figsize=(12, 6))
    figure.subplots_adjust(bottom=0.15, hspace=.5)
    result.plot_operator_counts(figure, title="Operator diagnostics")
    _, ax = plt.subplots(figsize=(12, 6))
    result.plot_objectives(ax, "Objective values")
    plt.show()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="ALNS Configuration")
    parser.add_argument("file_path", help="Path to the JSON or MSGPACK file to load")
    parser.add_argument("--initial-state-gen", choices=["hot-edges", "first-edges", "ilp"], default="hot-edges", help="Initial state generation method")
    parser.add_argument("--select", choices=["random", "roulette-wheel", "roulette-wheel-segmented"], default="random", help="Select method")
    parser.add_argument("--accept", choices=["hill-climbing", "late-acceptance-hill-climbing", "simulated-annealing"], default="hill-climbing", help="Accept method")
    parser.add_argument("--max-runtime", type=int, default=60, help="Max runtime of ALNS")
    parser.add_argument("--until-objective", type=int, default=None, help="Run ALNS until this objective is reached")

    args = parser.parse_args()

    aeg, critical_cycles = load_aeg(args.file_path)
    run_alns(aeg, critical_cycles, args.initial_state_gen, args.select, args.accept, args.max_runtime, args.until_objective)
