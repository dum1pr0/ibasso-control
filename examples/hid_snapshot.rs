//! Diagnostic: connects through the same worker the app uses, shows read progress, then prints the state.
//! It only sends the read-only connect queries.
use hidapi::HidApi;
use ibasso_control::hid::{self, Command};
use std::time::Duration;

fn main() {
    let mut api = HidApi::new().expect("hidapi");
    let entry = hid::scan(&mut api).into_iter().find(|e| e.family.is_some()).expect("no supported device attached");
    let handle = hid::open(&api, &entry, entry.family.unwrap()).expect("open");
    let started = std::time::Instant::now();
    let mut steps = 0;
    let result = loop {
        match handle.rx.recv_timeout(Duration::from_secs(10)) {
            Ok(snapshot) if snapshot.progress.is_some() => steps += 1,
            other => break other,
        }
    };
    println!("{}: {steps} progress updates, ready after {:?}", entry.product, started.elapsed());
    match result {
        Ok(snapshot) => {
            let mut values: Vec<_> = snapshot.values.iter().map(|(k, v)| format!("{k:?}={v}")).collect();
            values.sort();
            println!(
                "ready={} firmware={:?} peq_loaded={} error={:?} controls={}",
                snapshot.ready,
                snapshot.firmware,
                snapshot.peq.is_some(),
                snapshot.error,
                values.len()
            );
            println!("{}", values.join(" "));
        }
        Err(e) => println!("no state published: {e}"),
    }
    let _ = handle.tx.send(Command::Close);
    std::thread::sleep(Duration::from_millis(200));
}
