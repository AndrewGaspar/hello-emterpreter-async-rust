#[macro_use]
extern crate stdweb;
extern crate node_rs;

use std::os::raw::c_void;

use node_rs::{cluster, process, Promise};

fn wait(promise: Promise) {
    unsafe {
        block_on_promise(promise.as_ref().as_raw());
    }
}

extern "C" {
    fn block_on_promise(handle: i32) -> c_void;
}

fn set_timeout<F: 'static + FnOnce()>(callback: F, ms: i32) {
    js! {
        @(no_return)
        setTimeout(function() {
            let callback = @{stdweb::Once(callback)};
            callback();
        }, @{ms});
    };
}

fn rust_sleep(ms: i32) {
    wait(Promise::new(move |resolve, _| {
        set_timeout(
            move || {
                resolve.complete();
            },
            ms,
        );
    }));
}

fn main() {
    stdweb::initialize();

    let p = process();

    if let Some(worker) = cluster::worker() {
        assert!(cluster::is_worker());

        println!("I'm not the master! pid = {}", p.pid());

        rust_sleep(1000);

        worker.disconnect();
    } else {
        println!("I'm the master! pid = {}", p.pid());

        let args: Vec<_> = std::env::args().collect();

        let num_procs = if args.len() >= 2 {
            args[1].parse().expect("First argument must be an integer.")
        } else {
            4
        };

        let workers: Vec<_> = (0..num_procs).map(|_| cluster::fork()).collect();

        let promises: Vec<_> = workers
            .iter()
            .cloned()
            .map(|worker| {
                Promise::new(move |resolve, _| {
                    worker.on_exit(move |_, _| {
                        resolve.complete();
                    });
                })
            })
            .collect();

        wait(Promise::all(&promises));

        println!("Master exiting...");
    }
}
