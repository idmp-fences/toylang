# solution is of format sol.edge = (in,out); sol.placed = 1/0
import random
import numpy as np

class Problem:
    def __init__(self, edges, cycles, node_weights, edge_weights):
        self.edges = edges
        self.cycles = cycles
        self.node_weights = node_weights
        self.edge_weights = edge_weights
        self.graph = self.create_graph()

    def create_graph(self):
        graph = {}
        for edge in self.edges:
            if edge[0] not in graph:
                graph[edge[0]] = []
            graph[edge[0]].append(edge[1])
        return graph

    def get_node_weight(self, node):
        return self.node_weights.get(node, None)

    def get_edge_weight(self, edge_index):
        return self.edge_weights.get(edge_index, None)

    def display_graph(self):
        for node, edges in self.graph.items():
            print(f"Node {node} connects to {edges}")

    def display_cycles(self):
        print("Cycles in the graph:")
        for cycle in self.cycles:
            print(cycle)

class Solution:
    def __init__(self):
        self.edges = []  # List of tuples representing edges (in, out)
        self.placed = []  # List of 1/0 indicating if the edge is placed or not

    def check_solution():
        #make ILP call to check if solution is valid
        return True

class ALNS:
    def __init__(self, initial_solution, removal_heuristics, repair_heuristics):
        self.current_solution = initial_solution
        self.best_solution = initial_solution
        self.removal_heuristics = removal_heuristics
        self.repair_heuristics = repair_heuristics
        self.weights = {
            'removal': np.ones(len(removal_heuristics)),
            'repair': np.ones(len(repair_heuristics))
        }
        self.scores = {
            'removal': np.zeros(len(removal_heuristics)),
            'repair': np.zeros(len(repair_heuristics))
        }
        self.iterations = 1000  # Number of iterations
        self.adaptive_period = 100  # Period for updating weights

    def adapt_weights(self):
        for key in self.weights:
            total_score = np.sum(self.scores[key])
            if total_score > 0:
                self.weights[key] += self.scores[key] / total_score
            self.weights[key] /= np.sum(self.weights[key])
            self.scores[key] = np.zeros_like(self.scores[key])

    def select_heuristic(self, heuristic_type):
        if heuristic_type == 'removal':
            return random.choices(
                self.removal_heuristics, weights=self.weights['removal'], k=1
            )[0]
        elif heuristic_type == 'repair':
            return random.choices(
                self.repair_heuristics, weights=self.weights['repair'], k=1
            )[0]

    def evaluate_solution(self, solution):
        # Find out if we can use a multi-objective comparison or simply weighted function
        return sum(solution.placed)

    def run(self):
        for iteration in range(self.iterations):
            # Select and apply a removal heuristic
            removal_heuristic = self.select_heuristic('removal')
            partial_solution = removal_heuristic(self.current_solution)

            # Select and apply a repair heuristic
            repair_heuristic = self.select_heuristic('repair')
            new_solution = repair_heuristic(partial_solution)

            # Evaluate the new solution
            new_cost = self.evaluate_solution(new_solution)
            current_cost = self.evaluate_solution(self.current_solution)

            # Update current and best solutions
            if new_cost < current_cost:
                self.current_solution = new_solution
                if new_cost < self.evaluate_solution(self.best_solution):
                    self.best_solution = new_solution

            # Update scores based on the improvement
            improvement = current_cost - new_cost
            self.scores['removal'][self.removal_heuristics.index(removal_heuristic)] += improvement
            self.scores['repair'][self.repair_heuristics.index(repair_heuristic)] += improvement

            # Adapt weights periodically
            if iteration % self.adaptive_period == 0:
                self.adapt_weights()

        return self.best_solution

# Example usage with dummy heuristics
def random_removal_heuristic(solution):
    # Copy the current solution
    new_solution = Solution()
    new_solution.edges = solution.edges.copy()
    new_solution.placed = solution.placed.copy()

    # Find the indices of placed edges
    placed_indices = [i for i, placed in enumerate(new_solution.placed) if placed == 1]
    
    # Randomly select 50% of these indices to remove
    num_to_remove = len(placed_indices) // 2
    indices_to_remove = random.sample(placed_indices, num_to_remove)

    # Set the selected indices to 0
    for index in indices_to_remove:
        new_solution.placed[index] = 0

    return new_solution

def branch_bound_repair_heuristic(partial_solution):
    # Implement your repair heuristic
    return partial_solution

# Test case data
edges = [(0, 1), (2, 3), (4, 5), (5, 6), (6, 7), (8, 9), (10, 11), (0, 7), (7, 0),
         (0, 10), (10, 0), (1, 5), (5, 1), (1, 9), (9, 1), (3, 4), (4, 3), (6, 2),
         (2, 6), (6, 8), (8, 6), (6, 11), (11, 6), (9, 5), (5, 9), (10, 7), (7, 10)]

cycles = [[6, 7, 10, 11]]

node_weights = {
    0: ('Write', "t1", "t"),
    1: ('Write', "t1", "y"),
    2: ('Read', "t2", "z"),
    3: ('Write', "t2", "x"),
    4: ('Read', "t3", "x"),
    5: ('Read', "t3", "y"),
    6: ('Write', "t3", "z"),
    7: ('Read', "t3", "t"),
    8: ('Read', "t4", "z"),
    9: ('Write', "t4", "y"),
    10: ('Write', "t5", "t"),
    11: ('Read', "t5", "z"),
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

# Create problem instance
problem = Problem(edges, cycles, node_weights, edge_weights)

# Initial solution (Example, should be created based on problem specifics)
initial_solution = Solution()
initial_solution.edges = edges
initial_solution.placed = [1] * len(edges)

# Define your removal and repair heuristics
removal_heuristics = [random_removal_heuristic]
repair_heuristics = [dummy_repair_heuristic]

# Create ALNS instance
alns = ALNS(initial_solution, removal_heuristics, repair_heuristics)

# Run the ALNS algorithm
best_solution = alns.run()
print("Best Solution Edges:", best_solution.edges)
print("Best Solution Placed:", best_solution.placed)