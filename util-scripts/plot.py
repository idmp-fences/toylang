import pandas as pd
import matplotlib.pyplot as plt
import numpy as np


# Data
data = {
    "file_name": [
        "programs/cycles/lamport-2.msgpack", "programs/cycles/lamport-3.msgpack", 
        "programs/cycles/lamport-4.msgpack", "programs/cycles/lamport-5.msgpack", 
        "programs/cycles/peterson-2.msgpack", "programs/cycles/peterson-3.msgpack", 
        "programs/cycles/peterson-4.msgpack"
    ],
    "ilp_solve_time": [0.003, 0.052, 2.403, 104.73, 0.003, 0.010, 2.859],
    "alns_ilp_start": [0.007, 0.059, 2.427, 125.212, 0.004, 0.024, 3.615],
    "alns_hot_edges_start": [0.008, 0.007, 0.369, 18.951, 0.001, 0.001, 0.302]
}

# Convert to DataFrame
df = pd.DataFrame(data)

# Extract program names
df["program_name"] = df["file_name"].apply(lambda x: x.split('/')[-1].split('.')[0])

# Plotting
bar_width = 0.2
index = np.arange(len(df))


# Calculate relative times
df['rel_alns_ilp_start'] = df['alns_ilp_start'] / df['ilp_solve_time']
df['rel_alns_hot_edges_start'] = df['alns_hot_edges_start'] / df['ilp_solve_time']


colors = ['#ffba08', '#f48c06', '#dc2f02']  # Specify the colors for consistency

# Extracting Lamport and Peterson data
lamport_df = df[df['program_name'].str.contains('lamport')]
peterson_df = df[df['program_name'].str.contains('peterson')]

# Plotting
fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(16, 6), sharey=True)


# Adding a dashed line at 1.0
ax1.axhline(y=1.0, color='black', linestyle='--', alpha=0.6)
ax2.axhline(y=1.0, color='black', linestyle='--', alpha=0.6)

# Plot for Lamport
bar_width = 0.2
index_lamport = np.arange(len(lamport_df))

bar1 = ax1.bar(index_lamport, np.ones(len(lamport_df)), bar_width, label='ILP', color=colors[0])
bar2 = ax1.bar(index_lamport + bar_width, lamport_df['rel_alns_ilp_start'], bar_width, label='ALNS-ILP', color=colors[1])
bar3 = ax1.bar(index_lamport + 2 * bar_width, lamport_df['rel_alns_hot_edges_start'], bar_width, label='ALNS-HOT', color=colors[2])

# Adding labels on top of the bars for Lamport
for i in range(len(lamport_df)):
    ax1.text(index_lamport[i], 1.05, f'{lamport_df["ilp_solve_time"].iloc[i]:.2f}s', ha='center', va='bottom')
    ax1.text(index_lamport[i] + bar_width, lamport_df['rel_alns_ilp_start'].iloc[i] + 0.05, f'{lamport_df["alns_ilp_start"].iloc[i]:.2f}s', ha='center', va='bottom')
    ax1.text(index_lamport[i] + 2 * bar_width, lamport_df['rel_alns_hot_edges_start'].iloc[i] + 0.05, f'{lamport_df["alns_hot_edges_start"].iloc[i]:.2f}s', ha='center', va='bottom')

# Plot for Peterson
index_peterson = np.arange(len(peterson_df))

bar1 = ax2.bar(index_peterson, np.ones(len(peterson_df)), bar_width, color=colors[0])
bar2 = ax2.bar(index_peterson + bar_width, peterson_df['rel_alns_ilp_start'], bar_width, color=colors[1])
bar3 = ax2.bar(index_peterson + 2 * bar_width, peterson_df['rel_alns_hot_edges_start'], bar_width, color=colors[2])

# Adding labels on top of the bars for Peterson
for i in range(len(peterson_df)):
    ax2.text(index_peterson[i], 1.05, f'{peterson_df["ilp_solve_time"].iloc[i]:.2f}s', ha='center', va='bottom')
    ax2.text(index_peterson[i] + bar_width, peterson_df['rel_alns_ilp_start'].iloc[i] + 0.05, f'{peterson_df["alns_ilp_start"].iloc[i]:.2f}s', ha='center', va='bottom')
    ax2.text(index_peterson[i] + 2 * bar_width, peterson_df['rel_alns_hot_edges_start'].iloc[i] + 0.05, f'{peterson_df["alns_hot_edges_start"].iloc[i]:.2f}s', ha='center', va='bottom')


# Adding labels and title
ax1.set_ylabel('Relative Solving Time')
# ax1.set_title('Lamport Programs')
ax1.set_xticks(index_lamport + bar_width)
ax1.set_xticklabels(lamport_df['program_name'], rotation=45)

# ax2.set_title('Peterson Programs')
ax2.set_xticks(index_peterson + bar_width)
ax2.set_xticklabels(peterson_df['program_name'], rotation=45)

# Combined legend
# fig.suptitle('Relative execution time of different programs')
# handles, labels = ax1.get_legend_handles_labels()
# fig.legend(handles, labels, loc='upper center', ncol=3)
ax1.legend(loc='upper right')


# Display the plot
plt.tight_layout(rect=[0, 0, 1, 0.95])
plt.show()
