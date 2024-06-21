import alns
from alns.Result import Statistics
from matplotlib import pyplot as plt

# aggregated results from running random-depth-4
dest = {'biggest_cycle': [85, 0, 69, 205], 'random_10': [31, 0, 175, 141], 'cold_fences': [127, 0, 68, 185], 'hot_fences': [129, 0, 66, 176], 'random_30': [61, 0, 104, 193], 'same_cycle': [73, 0, 85, 196]}
repair = {'ilp_partial': [154, 0, 215, 41], 'hot_fences': [70, 0, 121, 171], 'unbroken': [12, 0, 28, 296], 'in_degrees': [1, 0, 1, 303], 'ilp_full': [248, 0, 163, 0], 'most_cycles': [21, 0, 39, 285]}

stats = Statistics()
stats._destroy_operator_counts = dest
stats._repair_operator_counts = repair

res = alns.Result(best=None, statistics=stats)

figure = plt.figure("operator_counts", figsize=(12, 6))
figure.subplots_adjust(bottom=0.15, hspace=.5)
res.plot_operator_counts(figure, title="Operator diagnostics")
_, ax = plt.subplots(figsize=(12, 6))
res.plot_objectives(ax, "Objective values")
plt.show()