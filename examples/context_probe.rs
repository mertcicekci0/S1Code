//! Offline mechanics probe, never a provider speed/cost benchmark.
use anyhow::{Result, ensure};
use s1code::{context, domain::*, session::Store};
use serde_json::json;
use std::time::Instant;

fn main() -> Result<()> {
    let mut trials = vec![];
    for count in [16, 64, 256] {
        let root = tempfile::tempdir()?;
        let home = tempfile::tempdir()?;
        let (store, mut original) = Store::create(
            home.path(),
            root.path(),
            "Inspect a boundary regression; preserve diagnostics".into(),
            RunConfig::default(),
        )?;
        for i in 0..count {
            let bytes = format!("result {i}\n{}", "historical evidence line\n".repeat(512));
            let artifact = store.put(
                bytes.as_bytes(),
                "offline_probe",
                &i.to_string(),
                "fixture-revision",
                vec![],
            )?;
            original.context.push(ContextItem {
                artifact,
                action: Action::List,
                pinned: false,
                evicted: false,
                diagnostic: false,
            });
        }
        let before = context::render(&original, &store)?.to_string().len();
        original.config.context_bytes = before / 3;
        let eligible = context::eligible(&original);
        let full_state = context::excerpts(&original, &store, &eligible)?
            .to_string()
            .len();
        let focused_bytes: usize = eligible
            .chunks(8)
            .map(|batch| context::excerpts(&original, &store, batch).map(|v| v.to_string().len()))
            .collect::<Result<Vec<_>>>()?
            .iter()
            .sum();
        for repeat in 0..5 {
            let mut s = original.clone();
            let start = Instant::now();
            let dropped = context::compact(&mut s, &store, None)?;
            let elapsed = start.elapsed().as_micros();
            let after = context::render(&s, &store)?.to_string().len();
            ensure!(
                !dropped.is_empty() && after <= s.config.context_bytes,
                "compaction did not fit"
            );
            let expected = store.get(&dropped[0])?;
            let recovered = context::rehydrate(&mut s, &dropped[0], &store)?;
            ensure!(recovered == expected, "rehydration changed bytes");
            trials.push(json!({"artifacts":count,"repeat":repeat,"active_bytes_before":before,"active_bytes_after":after,"evicted_artifacts":dropped.len(),"compaction_us":elapsed,"exact_rehydration":true,"retention_state_bytes_focused":focused_bytes,"retention_state_bytes_previous_layout":full_state * eligible.len().div_ceil(8)}));
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "mode":"OFFLINE MECHANICS — no inference, no provider performance claim",
            "version":env!("CARGO_PKG_VERSION"), "os":std::env::consts::OS,
            "architecture":std::env::consts::ARCH,"hardware_threads":std::thread::available_parallelism().ok().map(|n|n.get()),
            "provider_calls":0,"tokens":null,"cost":null,
            "notes":"Wall samples cover local compaction only, exclude fixture writes and rehydration; repeated runs share filesystem caches. Previous layout is calculated repeated-all-excerpts payload, not an external implementation benchmark.",
            "trials":trials
        }))?
    );
    Ok(())
}
