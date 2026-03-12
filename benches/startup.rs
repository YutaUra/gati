use criterion::{criterion_group, criterion_main, Criterion};

use gati::git_status::GitStatus;
use gati::tree::FileTreeModel;

/// Resolve the benchmark target directory.
/// Defaults to the current directory (gati repo, small).
/// Set `GATI_BENCH_REPO` for a large-repo benchmark, e.g.:
///
///   GATI_BENCH_REPO=~/src/nixpkgs cargo bench --bench startup
///
/// nixpkgs (~80k files) is recommended for realistic large-repo testing.
fn bench_dir() -> std::path::PathBuf {
    std::env::var("GATI_BENCH_REPO")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap())
}

fn bench_git_status_from_dir(c: &mut Criterion) {
    let dir = bench_dir();

    c.bench_function("GitStatus::from_dir", |b| {
        b.iter(|| {
            let _ = GitStatus::from_dir(criterion::black_box(&dir));
        });
    });
}

fn bench_file_tree_without_git(c: &mut Criterion) {
    let dir = bench_dir();

    c.bench_function("FileTreeModel::from_dir(None)", |b| {
        b.iter(|| {
            let _ = FileTreeModel::from_dir(criterion::black_box(&dir), None);
        });
    });
}

fn bench_file_tree_with_git(c: &mut Criterion) {
    let dir = bench_dir();

    // Pre-compute git status
    let gs = GitStatus::from_dir(&dir);

    c.bench_function("FileTreeModel::from_dir(Some(git_status))", |b| {
        b.iter(|| {
            let _ = FileTreeModel::from_dir(criterion::black_box(&dir), gs.clone());
        });
    });
}

criterion_group!(
    benches,
    bench_git_status_from_dir,
    bench_file_tree_without_git,
    bench_file_tree_with_git,
);
criterion_main!(benches);
