import sys
import pandas as pd
import matplotlib.pyplot as plt
import seaborn as sns

def plot_fct(csv_files, labels):
    plt.figure(figsize=(10, 6))
    for file, label in zip(csv_files, labels):
        df = pd.read_csv(file)
        sns.ecdfplot(df['fct_ns'] / 1e6, label=label) # Convert ns to ms

    plt.xlabel('Flow Completion Time (ms)')
    plt.ylabel('Cumulative Probability')
    plt.title('FCT Distribution (CDF)')
    plt.legend()
    plt.grid(True, alpha=0.3)
    plt.tight_layout()
    plt.savefig('fct_cdf.png')
    print("Saved fct_cdf.png")

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: python fct_distribution.py <csv1> <label1> [<csv2> <label2> ...]")
        sys.exit(1)

    files = sys.argv[1::2]
    labels = sys.argv[2::2]
    plot_fct(files, labels)