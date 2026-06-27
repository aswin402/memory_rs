use criterion::{criterion_group, criterion_main, Criterion, black_box};
use openmemory_rs::layers::semantic::SemanticMemory;
use openmemory_rs::layers::graph::GraphMemory;
use openmemory_rs::layers::MemoryScope;
use openmemory_rs::extraction::ContextCompressor;
use std::fs;

fn bench_semantic_insert(c: &mut Criterion) {
    let temp_dir = std::env::temp_dir();
    let db_path = temp_dir.join(format!("bench_semantic_insert_{}.db", uuid::Uuid::new_v4()));
    if db_path.exists() {
        let _ = fs::remove_file(&db_path);
    }
    let semantic = SemanticMemory::new(&db_path).unwrap();
    let scope = MemoryScope::default();

    c.bench_function("semantic_insert", |b| {
        let mut count = 0;
        b.iter(|| {
            count += 1;
            let text = format!("This is a benchmark fact number {}.", count);
            let _ = semantic.add_fact(&format!("fact-{}", count), &text, 0.8, &scope);
        })
    });

    let _ = fs::remove_file(&db_path);
}

fn bench_semantic_query(c: &mut Criterion) {
    let temp_dir = std::env::temp_dir();
    let db_path = temp_dir.join(format!("bench_semantic_query_{}.db", uuid::Uuid::new_v4()));
    if db_path.exists() {
        let _ = fs::remove_file(&db_path);
    }
    let semantic = SemanticMemory::new(&db_path).unwrap();
    let scope = MemoryScope::default();

    // Seed some data
    for i in 0..50 {
        let _ = semantic.add_fact(
            &format!("fact-{}", i),
            &format!("Rust is a safe systems programming language version {}.", i),
            0.9,
            &scope,
        );
    }

    c.bench_function("semantic_query", |b| {
        b.iter(|| {
            let _ = semantic.query_similar_facts(black_box("safe systems language"), 5, &scope);
        })
    });

    let _ = fs::remove_file(&db_path);
}

fn bench_context_compression(c: &mut Criterion) {
    let compressor = ContextCompressor::new();
    let text = "First sentence about Rust compilation. Second sentence about Rust compiler speed. Third fluffy sentence. Fourth random sentence here. Fifth compiler optimization step. Sixth LLVM backend pass.";

    c.bench_function("context_compression", |b| {
        b.iter(|| {
            let _ = compressor.compress_by_ratio(black_box(text), 0.5);
        })
    });
}

fn bench_graph_traversal(c: &mut Criterion) {
    let temp_dir = std::env::temp_dir();
    let db_path = temp_dir.join(format!("bench_graph_traversal_{}.db", uuid::Uuid::new_v4()));
    if db_path.exists() {
        let _ = fs::remove_file(&db_path);
    }
    let graph = GraphMemory::new(&db_path).unwrap();
    let scope = MemoryScope::default();

    // Create a chain of nodes: A -> B -> C -> D -> E
    use openmemory_rs::layers::graph::{Entity, Relation};
    let entities = vec![
        Entity { name: "A".to_string(), entity_type: "Node".to_string(), observations: vec![] },
        Entity { name: "B".to_string(), entity_type: "Node".to_string(), observations: vec![] },
        Entity { name: "C".to_string(), entity_type: "Node".to_string(), observations: vec![] },
        Entity { name: "D".to_string(), entity_type: "Node".to_string(), observations: vec![] },
        Entity { name: "E".to_string(), entity_type: "Node".to_string(), observations: vec![] },
    ];
    let _ = graph.create_entities(entities, &scope);

    let relations = vec![
        Relation { from: "A".to_string(), to: "B".to_string(), relation_type: "next".to_string() },
        Relation { from: "B".to_string(), to: "C".to_string(), relation_type: "next".to_string() },
        Relation { from: "C".to_string(), to: "D".to_string(), relation_type: "next".to_string() },
        Relation { from: "D".to_string(), to: "E".to_string(), relation_type: "next".to_string() },
    ];
    let _ = graph.create_relations(relations, &scope);

    c.bench_function("graph_traversal_bfs", |b| {
        b.iter(|| {
            let _ = openmemory_rs::layers::graph_traversal::bfs_traverse(
                &graph,
                black_box("A"),
                black_box(3),
                &scope,
            );
        })
    });

    let _ = fs::remove_file(&db_path);
}

criterion_group!(
    benches,
    bench_semantic_insert,
    bench_semantic_query,
    bench_context_compression,
    bench_graph_traversal
);
criterion_main!(benches);
