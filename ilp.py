from typing import List, Dict, Any

import pulp


class Node:
    def __init__(self, id: int, node_type: str, thread: str, variable: str):
        self.id = id
        self.node_type = node_type
        self.thread = thread
        self.variable = variable

    def __repr__(self):
        return f"Node(id={self.id}, type={self.node_type}, thread={self.thread}, variable={self.variable})"

class Edge:
    def __init__(self, id: int, source: int, target: int, edge_type: str):
        self.id = id
        self.source = source
        self.target = target
        self.edge_type = edge_type

    def __repr__(self):
        return f"Edge(id={self.id}, source={self.source}, target={self.target}, type={self.edge_type})"

class AbstractEventGraph:
    def __init__(self, nodes: List[Dict[str, List[str]]], edges: List[List[Any]]):
        self.nodes = [self._parse_node(i, node) for i, node in enumerate(nodes)]
        self.edges = [Edge(i, *edge) for i, edge in enumerate(edges)]
        self.fences = []

    def _parse_node(self, id: int, node_dict: Dict[str, List[str]]) -> Node:
        node_type, values = next(iter(node_dict.items()))
        thread, variable = values
        return Node(id, node_type, thread, variable)

    def __repr__(self):
        return f"AbstractEventGraph(nodes={self.nodes}, edges={self.edges}, fences={self.fences})"


class CriticalCycle:
    def __init__(self, node_ids: List[int], edge_ids: List[int], aeg: AbstractEventGraph):
        self.nodes = [aeg.nodes[node_id] for node_id in node_ids]
        self.edges = [aeg.edges[edge_id] for edge_id in edge_ids]

    def __repr__(self):
        return f"CriticalCycle(nodes={self.nodes}, edges={self.edges})"


class ILPSolver:
    def __init__(self, aeg: AbstractEventGraph, critical_cycles: List[CriticalCycle]):
        self.aeg = aeg
        self.critical_cycles = critical_cycles

    def fence_placement(self):

        # Initialize the ILP problem
        prob = pulp.LpProblem("TSOFencePlacement", pulp.LpMinimize)

        # Set of possible fences
        possible_fences = {edge for cycle in self.critical_cycles for edge in cycle.edges}

        # Define the decision variables for each unique edge
        fences = {edge: pulp.LpVariable(f'f_{edge}', cat='Binary') for edge in possible_fences}

        # Objective function: Minimize the number of fences
        prob += pulp.lpSum(fences[edge] for edge in fences), "MinimizeFences"

        # Constraints: Ensure each critical cycle is broken
        for i, cycle in enumerate(self.critical_cycles):
            prob += pulp.lpSum(fences[edge] for edge in cycle.edges) >= 1, f"BreakCriticalCycle_{i}"

        # Already Placed Fences
        prob += pulp.lpSum(fences[edge] for edge in self.aeg.fences) >= len(self.aeg.fences)

        # Solve the problem
        prob.solve()

        # Update the aeg
        self.aeg.fences = list(edge for edge in fences if int(pulp.value(fences[edge])) == 1)
