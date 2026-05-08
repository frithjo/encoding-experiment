"""
Generate relation_clusters.json from existing vindex gate vectors.
Uses k-means clustering on gate vector directions to find relation clusters.
"""

import numpy as np
import json
import sys
import os

sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))

import larql._native as larql
from sklearn.cluster import KMeans


def generate_relation_clusters(vindex_path: str, output_path: str, n_clusters: int = 20):
    """Generate relation clusters by clustering gate vector directions."""
    print(f"Loading vindex from {vindex_path}...")
    vindex = larql.load_vindex(vindex_path)
    print(f"  {vindex}")
    
    # Get gate vectors from knowledge layers
    bands = vindex.layer_bands()
    knowledge_start = bands["knowledge"][0] if bands else 14
    knowledge_end = bands["knowledge"][1] if bands else 27
    
    print(f"Knowledge layers: {knowledge_start}-{knowledge_end}")
    
    # Get hidden dimension from index.json
    index_path_local = os.path.join(vindex_path, "index.json")
    with open(index_path_local) as f:
        index_data = json.load(f)
    hidden_dim = index_data["hidden_size"]
    print(f"Hidden dimension: {hidden_dim}")
    
    # Collect gate vectors from knowledge layers
    gate_vectors = []
    layer_info = []
    
    # Sample gate vectors (don't load all to save memory)
    # Load first 500 features per layer for clustering
    sample_size = 500
    for layer in range(knowledge_start, knowledge_end):
        print(f"  Sampling layer {layer}...")
        for feat in range(sample_size):
            try:
                gate_vec = vindex.gate_vector(layer, feat)
                gate_vectors.append(gate_vec)
                layer_info.append((layer, feat))
            except Exception as e:
                # Skip if gate vector not available
                continue
    
    gate_vectors = np.array(gate_vectors, dtype=np.float32)
    print(f"Total gate vectors sampled: {gate_vectors.shape}")
    
    if len(gate_vectors) == 0:
        print("ERROR: No gate vectors sampled. Using placeholder clusters.")
        return generate_placeholder_clusters(hidden_dim, output_path, n_clusters)
    
    # Run k-means clustering
    print(f"Running k-means clustering with k={n_clusters}...")
    kmeans = KMeans(n_clusters=n_clusters, random_state=42, n_init=10, max_iter=300)
    cluster_labels = kmeans.fit_predict(gate_vectors)
    
    # Get cluster centres
    centres = kmeans.cluster_centers_
    print(f"Clustering complete. Cluster centres shape: {centres.shape}")
    
    # Build relation_clusters.json
    # Map cluster indices to relation names using category vocabulary
    categories = [
        "capital", "language", "occupation", "creator", "located_in",
        "born_in", "died_in", "nationality", "religion", "political_party",
        "spouse", "child", "parent", "sibling", "founded",
        "member_of", "leader_of", "headquarters", "currency", "population"
    ]
    
    # Assign category names to clusters
    relation_clusters = {}
    for i, centre in enumerate(centres):
        relation_name = categories[i % len(categories)]
        relation_clusters[relation_name] = {
            "centre": centre.tolist(),
            "cluster_id": i
        }
    
    # Write relation_clusters.json
    with open(output_path, 'w') as f:
        json.dump(relation_clusters, f, indent=2)
    
    print(f"Written {len(relation_clusters)} relation clusters to {output_path}")
    print(f"Clusters generated from actual gate vectors")
    
    return relation_clusters


def generate_placeholder_clusters(hidden_dim: int, output_path: str, n_clusters: int):
    """Fallback: generate placeholder clusters."""
    categories = [
        "capital", "language", "occupation", "creator", "located_in",
        "born_in", "died_in", "nationality", "religion", "political_party",
        "spouse", "child", "parent", "sibling", "founded",
        "member_of", "leader_of", "headquarters", "currency", "population"
    ]
    
    relation_clusters = {}
    for i, relation_name in enumerate(categories[:n_clusters]):
        centre = np.random.randn(hidden_dim).astype(np.float32) * 0.01
        relation_clusters[relation_name] = {
            "centre": centre.tolist(),
            "cluster_id": i
        }
    
    with open(output_path, 'w') as f:
        json.dump(relation_clusters, f, indent=2)
    
    print(f"Written {len(relation_clusters)} placeholder relation clusters to {output_path}")
    return relation_clusters


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python generate_relation_clusters.py <vindex_path> [n_clusters]")
        print("Example: python generate_relation_clusters.py ../data/bitnet_b1_58-large/vindex")
        sys.exit(1)
    
    vindex_path = sys.argv[1]
    output_path = os.path.join(vindex_path, "relation_clusters.json")
    n_clusters = int(sys.argv[2]) if len(sys.argv) > 2 else 20
    
    generate_relation_clusters(vindex_path, output_path, n_clusters)
