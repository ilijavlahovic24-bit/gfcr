import sys
import pandas as pd
import matplotlib.pyplot as plt
import seaborn as sns

def plot_heatmap(csv_file):
    df = pd.read_csv(csv_file)

    # Use pivot_table instead of pivot to handle duplicate timestamps (multiple events at the same ns)
    # aggfunc='max' takes the highest utilization on that physical link at that moment.
    pivot = df.pivot_table(
        index='link_id',
        columns='time_ns',
        values='utilization_pct',
        aggfunc='max'
    )

    # Downsample time for cleaner plot if too many events
    if len(pivot.columns) > 500:
        step = len(pivot.columns) // 500
        pivot = pivot.iloc[:, ::step]

    plt.figure(figsize=(12, 8))
    sns.heatmap(pivot, cmap="YlOrRd", cbar_kws={'label': 'Utilization (%)'})
    plt.xlabel('Simulation Time (normalized)')
    plt.ylabel('Link ID')
    plt.title('Link Utilization Heatmap')
    plt.tight_layout()
    plt.savefig('utilization_heatmap.png')
    print("Saved utilization_heatmap.png")

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python heatmap.py <utilization_csv>")
        sys.exit(1)
    plot_heatmap(sys.argv[1])