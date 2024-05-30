import pulp


def find_fence_placement(edges, cycles, node_weights, edge_weights):
    """
    Function to find the optimal fence placement for given critical cycles using ILP.

    Parameters:
    critical_cycles (list of list of tuples): List of critical cycles, where each cycle is a list of edges (tuples).
    node_weights (dict): Dictionary mapping node indices to their weights, indicating if they are Read or Write operations.

    Returns:
    dict: Dictionary with edges as keys and binary values indicating whether a fence is placed (1) or not (0).
    """

    filtered_edges = find_relevant_edges(edges, cycles, node_weights, edge_weights)

    # Extract unique edges from all critical cycles
    unique_edges = set(filtered_edges)

    # Initialize the ILP problem
    prob = pulp.LpProblem("TSOFencePlacement", pulp.LpMinimize)

    # Define the decision variables for each unique edge
    fences = {edge: pulp.LpVariable(f'f_{edge}', cat='Binary') for edge in unique_edges}

    # Objective function: Minimize the number of fences
    prob += pulp.lpSum(fences[edge] for edge in fences), "MinimizeFences"

    # Constraints: Ensure each critical cycle is broken
    for i, cycle in enumerate(cycles):
        cycle_edges = []
        for i in range(len(cycle)):
            for j in range(len(cycle)):
                if i != j:
                    u = cycle[i]
                    v = cycle[j]
                    cycle_edges.append((u, v))
        prob += pulp.lpSum(fences[edge] for edge in cycle_edges if edge in fences) >= 1, f"BreakCriticalCycle_{i}"
    # Solve the problem
    prob.solve()

    # Prepare the results
    result = {edge: int(pulp.value(fences[edge])) for edge in fences}

    return result


def find_relevant_edges(edges, cycles, node_weights, edge_weights):
    relevant_edges = []
    edge_index_map = {edge: i for i, edge in enumerate(edges)}

    # Create adjacency list for ProgramOrder edges
    program_order_graph = {}
    for i, edge in enumerate(edges):
        if edge_weights[i] == 'ProgramOrder':
            if edge[0] not in program_order_graph:
                program_order_graph[edge[0]] = []
            program_order_graph[edge[0]].append(edge[1])

    def has_program_order_path(u, v):
        # Perform DFS to check for path from u to v using ProgramOrder edges
        stack = [u]
        visited = set()

        while stack:
            node = stack.pop()
            if node == v:
                return True
            if node not in visited:
                visited.add(node)
                if node in program_order_graph:
                    stack.extend(program_order_graph[node])
        return False

    for cycle in cycles:
        for i in range(len(cycle)):
            for j in range(len(cycle)):
                if i != j:
                    u = cycle[i]
                    v = cycle[j]
                    edge = (u, v)
                    if edge in edge_index_map:
                        edge_index = edge_index_map[edge]
                        if (node_weights[u][0] == 'Write' and node_weights[v][0] == 'Read' and edge_weights[
                            edge_index] == 'ProgramOrder'):
                            relevant_edges.append(edge)
                    elif has_program_order_path(u, v):
                        if node_weights[u][0] == 'Write' and node_weights[v][0] == 'Read':
                            relevant_edges.append(edge)
    return relevant_edges


if __name__ == "__main__":
    # Example usage
    edges = [(0, 1), (2, 3), (4, 5), (5, 6), (6, 7), (8, 9), (10, 11), (0, 7), (7, 0),
         (0, 10), (10, 0), (1, 5), (5, 1), (1, 9), (9, 1), (3, 4), (4, 3), (6, 2),
         (2, 6), (6, 8), (8, 6), (6, 11), (11, 6), (9, 5), (5, 9), (10, 7), (7, 10)]
    cycles = [[6, 7, 10, 11]]
    node_weights = {
        0: ('Write',
            "t1",
            "t",
        ),
        1: ('Write',
            "t1",
            "y",
        ),
        2: ('Read',
            "t2",
            "z",
        ),
        3: ('Write',
            "t2",
            "x",
        ),
        4: ('Read',
            "t3",
            "x",
        ),
        5: ('Read',
            "t3",
            "y",
        ),
        6: ('Write',
            "t3",
            "z",
        ),
        7: ('Read',
            "t3",
            "t",
        ),
        8: ('Read',
            "t4",
            "z",
        ),
        9: ('Write',
            "t4",
            "y",
        ),
        10: ('Write',
            "t5",
            "t",
        ),
        11: ('Read',
            "t5",
            "z",
        ),
    }

    edge_weights = {
        0: 'ProgramOrder',
        1: 'ProgramOrder',
        2: 'ProgramOrder',
        3: 'ProgramOrder',
        4: 'ProgramOrder',
        5: 'ProgramOrder',
        6: 'ProgramOrder',
        7: 'Competing',
        8: 'Competing',
        9: 'Competing',
        10: 'Competing',
        11: 'Competing',
        12: 'Competing',
        13: 'Competing',
        14: 'Competing',
        15: 'Competing',
        16: 'Competing',
        17: 'Competing',
        18: 'Competing',
        19: 'Competing',
        20: 'Competing',
        21: 'Competing',
        22: 'Competing',
        23: 'Competing',
        24: 'Competing',
        25: 'Competing',
        26: 'Competing',
    }
    print(find_relevant_edges(edges, cycles, node_weights, edge_weights))
    fence_placement = find_fence_placement(edges, cycles, node_weights, edge_weights)
    print("Fences placed at edges:")
    for edge, placed in fence_placement.items():
        if placed == 1:
            print(f"  - Edge {edge}")

    print("Total number of fences:", sum(fence_placement.values()))
