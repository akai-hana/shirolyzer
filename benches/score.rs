use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

use rand::SeedableRng;
use shirolyzer::evaluate::{flat_share_from_config, perm_of, Evaluator};
use shirolyzer::search::{anneal, delta_swap_cost, quench, quench_generic, DeltaTables};
use shirolyzer::{conditions, config, corpus, gates, geometry, scorer};

const SHIRO: &str = "cntgf;oehkrplsmuazdixjqvw,.b/y";

fn main() {
    let check = std::env::var_os("SHIROLYZER_BENCH_ASSERT").is_some();
    let cfg = config::load(None).expect("config");
    let geom = geometry::Geometry::new();
    let model = config::build_model(&cfg).expect("model");
    let gates = gates::Gates::from_config(&cfg);
    let conds = conditions::Conditions::from_config(&cfg, &geom);
    let start = Instant::now();
    let tables = scorer::Tables::build(&model, &geom, &cfg, &gates, &conds).expect("tables");
    let tables_ms = start.elapsed().as_secs_f64() * 1e3;
    println!("tables build: {:.3} ms", tables_ms);
    let paths: Vec<PathBuf> = cfg
        .primary_corpus
        .iter()
        .map(|e| cfg.corpus_path(e))
        .collect();
    let corpus = corpus::load_many(&paths).expect("corpus");
    let perm = perm_of(SHIRO, &cfg.punctuation).expect("perm");
    let (w_balance, w_hand) = cfg.balance_weights();
    let ev = Evaluator::new(
        &corpus,
        &tables,
        &model,
        &cfg.punctuation,
        flat_share_from_config(&cfg),
        w_balance,
        w_hand,
    )
    .expect("evaluator");

    let start = Instant::now();
    let delta_tables = DeltaTables::build(&ev);
    let delta_build_ms = start.elapsed().as_secs_f64() * 1e3;
    println!("delta tables build: {:.3} ms", delta_build_ms);

    let n = 2_000;
    let start = Instant::now();
    for _ in 0..n {
        black_box(ev.score(black_box(&perm)));
    }
    let elapsed = start.elapsed().as_secs_f64();
    let score_us = elapsed / n as f64 * 1e6;
    println!(
        "full score: {:.2} us/op, {:.0} ops/s",
        score_us,
        n as f64 / elapsed
    );

    let reps = 200;
    let start = Instant::now();
    black_box(delta_swap_cost(&ev, &perm, reps));
    let elapsed = start.elapsed().as_secs_f64();
    let ops = reps * 30 * 29 / 2;
    let swap_ns = elapsed / ops as f64 * 1e9;
    println!(
        "delta swap: {:.0} ns/op, {:.0} ops/s",
        swap_ns,
        ops as f64 / elapsed
    );

    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(1);
    let start = Instant::now();
    let (_, _) = quench(&ev, &delta_tables, &perm, ev.score(&perm), &mut rng);
    let quench_ms = start.elapsed().as_secs_f64() * 1e3;
    println!("quench: {:.3} ms", quench_ms);

    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(1);
    let start = Instant::now();
    let (_, _) = quench_generic(&ev, &perm, ev.score(&perm), &mut rng);
    let generic_ms = start.elapsed().as_secs_f64() * 1e3;
    println!("full-score quench: {:.3} ms", generic_ms);

    let iters = 10_000;
    let start = Instant::now();
    let (_, _) = anneal(&ev, Some(&perm), iters, 0.35, 0.002, 7).expect("anneal");
    let elapsed = start.elapsed().as_secs_f64();
    let anneal_rate = iters as f64 / elapsed;
    println!(
        "anneal: {:.3} ms for {} iters ({:.0} iters/s)",
        elapsed * 1e3,
        iters,
        anneal_rate
    );

    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(1);
    let start = Instant::now();
    let candidate = ev.perturb(&perm, &mut rng, 6);
    let (candidate, _score) = quench(
        &ev,
        &delta_tables,
        &candidate,
        ev.score(&candidate),
        &mut rng,
    );
    let (candidate, score) = anneal(&ev, Some(&candidate), 5_000, 0.35, 0.002, 7).expect("anneal");
    let _ = quench(&ev, &delta_tables, &candidate, score, &mut rng);
    let ils_ms = start.elapsed().as_secs_f64() * 1e3;
    println!("ILS round: {:.3} ms", ils_ms);

    if check {
        assert!(
            tables_ms < 2_000.0,
            "tables build regressed: {tables_ms} ms"
        );
        assert!(
            delta_build_ms < 2_000.0,
            "delta tables build regressed: {delta_build_ms} ms"
        );
        assert!(score_us < 1_000.0, "full score regressed: {score_us} us/op");
        assert!(swap_ns < 200_000.0, "delta swap regressed: {swap_ns} ns/op");
        assert!(quench_ms < 10_000.0, "quench regressed: {quench_ms} ms");
        assert!(
            generic_ms < 30_000.0,
            "full-score quench regressed: {generic_ms} ms"
        );
        assert!(
            anneal_rate > 5_000.0,
            "anneal regressed: {anneal_rate} iters/s"
        );
        assert!(ils_ms < 20_000.0, "ILS round regressed: {ils_ms} ms");
        println!("benchmark ceilings passed");
    }
}
