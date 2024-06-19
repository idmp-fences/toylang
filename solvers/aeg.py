from typing import Any, Dict, List


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

    def _parse_node(self, id: int, node_dict: Dict[str, List[str]]) -> Node:
        node_type, values = next(iter(node_dict.items()))
        thread, variable = values
        return Node(id, node_type, thread, variable)

    def __repr__(self):
        return f"AbstractEventGraph(nodes={self.nodes}, edges={self.edges})"


class CriticalCycle:
    def __init__(self, node_ids: List[int], edge_ids: List[int], aeg: AbstractEventGraph):
        self.nodes = [aeg.nodes[node_id] for node_id in node_ids]
        self.edges = [aeg.edges[edge_id] for edge_id in edge_ids]

    def __repr__(self):
        return f"CriticalCycle(nodes={self.nodes}, edges={self.edges})"
