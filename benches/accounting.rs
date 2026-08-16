//! Accounting benchmarks across hierarchy depths and contended workers.

#![allow(missing_docs)]

use std::hint::black_box;
use std::sync::Arc;
use std::thread;

use budget_context::{Budget, Resource};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

fn budget_at_depth(resource: &Resource, depth: usize) -> Budget {
    let mut budget = Budget::builder()
        .limit(resource.clone(), u64::MAX)
        .build()
        .unwrap();
    for _ in 1..depth {
        budget = budget
            .child()
            .limit(resource.clone(), u64::MAX)
            .build()
            .unwrap();
    }
    budget
}

fn hierarchy_benchmarks(criterion: &mut Criterion) {
    let resource = Resource::new("bench.units").unwrap();
    let mut group = criterion.benchmark_group("hierarchy");
    for depth in [1, 2, 5, 10] {
        let budget = budget_at_depth(&resource, depth);
        group.bench_with_input(
            BenchmarkId::new("reserve_drop", depth),
            &depth,
            |bench, _| {
                bench.iter(|| {
                    let permit = budget.reserve(black_box(&resource), black_box(1)).unwrap();
                    let _ = black_box(permit);
                });
            },
        );
    }
    group.finish();
}

fn multi_resource_benchmark(criterion: &mut Criterion) {
    let one = Resource::new("bench.one").unwrap();
    let two = Resource::new("bench.two").unwrap();
    let three = Resource::new("bench.three").unwrap();
    let budget = Budget::builder()
        .limit(one.clone(), u64::MAX)
        .limit(two.clone(), u64::MAX)
        .limit(three.clone(), u64::MAX)
        .build()
        .unwrap();

    criterion.bench_function("reserve_many_drop", |bench| {
        bench.iter(|| {
            let permit = budget
                .reserve_many([
                    (black_box(&one), 1),
                    (black_box(&two), 2),
                    (black_box(&three), 3),
                ])
                .unwrap();
            let _ = black_box(permit);
        });
    });
}

fn contention_benchmarks(criterion: &mut Criterion) {
    let resource = Resource::new("bench.units").unwrap();
    let mut group = criterion.benchmark_group("contention");
    group.sample_size(20);
    for workers in [1, 4, 16, 64] {
        group.bench_with_input(
            BenchmarkId::new("reserve_drop", workers),
            &workers,
            |bench, &workers| {
                bench.iter(|| {
                    let budget = Arc::new(budget_at_depth(&resource, 2));
                    let handles: Vec<_> = (0..workers)
                        .map(|_| {
                            let budget = budget.clone();
                            let resource = resource.clone();
                            thread::spawn(move || {
                                for _ in 0..100 {
                                    let _ = black_box(budget.reserve(&resource, 1).unwrap());
                                }
                            })
                        })
                        .collect();
                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    hierarchy_benchmarks,
    multi_resource_benchmark,
    contention_benchmarks
);
criterion_main!(benches);
