from typing import List

from aeg import AbstractEventGraph, CriticalCycle, Edge


class ProblemInstance:
    """
    Contains the abstract event graph for an instance, plus some extra information about 
    the critical cycles and potential fence placements.
    """
    def __init__(self, aeg: AbstractEventGraph, critical_cycles: List[CriticalCycle]):
        self.aeg = aeg
        self.critical_cycles = critical_cycles
        self.fences: List[Edge] = []

        # The number of cc's that each edge is involved in. Higher counts indicate 'hot' edges
        self.edge_cc_count = {}

        for cycle in critical_cycles:
            for edge in cycle.edges:
                if edge in self.edge_cc_count:
                    self.edge_cc_count[edge] += 1
                else:
                    self.edge_cc_count[edge] = 0

        # All of the potential fence placements combined
        self.potential_fences = set(self.edge_cc_count.keys())


class ProblemState:
    """
        fences: List with edges where fences are placed
    """

    def __init__(self, instance: ProblemInstance):
        self.instance = instance
        self.fences: List[Edge] = []

    def copy(self):
        # Perform a deep copy only of the fences. The instance itself does not need to be copied
        copy = ProblemState(self.instance)
        copy.fences = self.fences.copy()
        return copy

    def objective(self) -> float:
        return len(self.fences)

    def get_context(self):
        # TODO implement a method returning a context vector. This is only
        #  needed for some context-aware bandit selectors from MABWiser;
        #  if you do not use those, this default is already sufficient!
        return None