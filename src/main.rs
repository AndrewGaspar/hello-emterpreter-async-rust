#[macro_use]
extern crate stdweb;

#[macro_use]
extern crate stdweb_derive;

use std::os::raw::c_void;

use stdweb::unstable::TryInto;
use stdweb::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Cluster(stdweb::Reference);

impl Cluster {
    pub fn module() -> Self {
        Cluster(
            js! {
                return require("cluster");
            }.into_reference()
                .unwrap(),
        )
    }

    pub fn is_master(&self) -> bool {
        match js! {
            return @{&self.0}.isMaster;
        } {
            stdweb::Value::Bool(b) => b,
            _ => false,
        }
    }

    pub fn is_worker(&self) -> bool {
        match js! {
            return @{&self.0}.isWorker;
        } {
            stdweb::Value::Bool(b) => b,
            _ => false,
        }
    }

    pub fn worker(&self) -> Option<Worker> {
        (js! {
            return @{&self.0}.worker;
        }).into_reference()
            .map(|worker| Worker(worker))
    }

    pub fn fork(&self) -> Worker {
        Worker(
            js! {
                return @{&self.0}.fork();
            }.into_reference()
                .unwrap(),
        )
    }

    pub fn on_exit<F: 'static + Fn(Worker, i32, Option<&str>) -> ()>(&self, callback: F) {
        let on_exit_callback =
            move |worker: stdweb::Value, code: stdweb::Value, signal: stdweb::Value| {
                let worker = Worker(worker.into_reference().unwrap());
                let code = code.try_into().unwrap();
                let signal = (&signal).try_into().ok();
                callback(worker, code, signal);
            };

        js! {
            @{&self.0}.on("exit", (worker, code, signal) => {
                let on_exit_callback = @{on_exit_callback};
                on_exit_callback(worker, code, signal);
            });
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, ReferenceType)]
#[reference(instance_of = "cluster.Worker")]
struct Worker(stdweb::Reference);

impl Worker {
    pub fn process(&self) -> Process {
        Process::from_reference(
            js! {
                return @{&self.0}.process;
            }.into_reference()
                .unwrap(),
        )
    }

    pub fn disconnect(&self) {
        js! {
            @{&self.0}.disconnect();
        };
    }

    pub fn on_exit<F: 'static + FnOnce(i32, Option<&str>) -> ()>(&self, callback: F) {
        let on_exit_callback = move |code: stdweb::Value, signal: stdweb::Value| {
            let code = code.try_into().unwrap();
            let signal = (&signal).try_into().ok();
            callback(code, signal);
        };

        js! {
            @{&self.0}.on("exit", (code, signal) => {
                let on_exit_callback = @{stdweb::Once(on_exit_callback)};
                on_exit_callback(code, signal);
            });
        }
    }
}

struct Process(stdweb::Reference);

impl Process {
    pub fn current() -> Process {
        Process(js! { return process; }.into_reference().unwrap())
    }

    pub fn from_reference(value: stdweb::Reference) -> Self {
        Process(value)
    }

    pub fn pid(&self) -> i32 {
        match js! {
            return @{&self.0}.pid;
        } {
            Value::Number(x) => x.try_into().unwrap(),
            _ => panic!("Type of process.pid unexpected!"),
        }
    }

    pub fn exit() -> ! {
        js! {
            process.exit();
        };

        panic!("Process didn't exit when it should have!");
    }
}

#[derive(Clone, Debug, PartialEq, Eq, ReferenceType)]
#[reference(instance_of = "Promise")]
struct Promise(stdweb::Reference);

impl Promise {
    pub fn new<F>(user_callback: F) -> Self
    where
        F: 'static + FnOnce(PromiseCallback, PromiseCallback),
    {
        let callback = move |resolve: stdweb::Value, reject: stdweb::Value| {
            user_callback(
                PromiseCallback(resolve.into_reference().unwrap()),
                PromiseCallback(reject.into_reference().unwrap()),
            );
        };

        Promise(
            js! {
                return new Promise(function(resolve, reject) {
                    let callback = @{stdweb::Once(callback)};
                    callback(resolve, reject);
                });
            }.into_reference()
                .unwrap(),
        )
    }

    pub fn all(promises: &[Promise]) -> Promise {
        Promise(
            js! {
                return Promise.all(@{promises});
            }.into_reference()
                .unwrap(),
        )
    }

    pub fn wait(&self) {
        unsafe {
            block_on_promise(self.0.as_raw());
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, ReferenceType)]
#[reference(instance_of = "Function")]
struct PromiseCallback(stdweb::Reference);

impl PromiseCallback {
    pub fn complete(self) {
        js! {
            @(no_return)
            let completion = @{self.0};
            completion();
        }
    }

    pub fn with<JS: stdweb::JsSerialize>(self, value: JS) {
        js! {
            @(no_return)
            let completion = @{self.0};
            completion(@{value});
        }
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
    Promise::new(move |resolve, _| {
        set_timeout(
            move || {
                resolve.complete();
            },
            ms,
        );
    }).wait();
}

fn main() {
    stdweb::initialize();

    let p = Process::current();

    let cluster = Cluster::module();

    if let Some(worker) = cluster.worker() {
        assert!(cluster.is_worker());

        println!("I'm not the master! pid = {}", p.pid());

        rust_sleep(1000);

        worker.disconnect();
    } else {
        println!("I'm the master! pid = {}", Process::current().pid());

        let args: Vec<_> = std::env::args().collect();

        let num_procs = if args.len() >= 2 {
            args[1].parse().expect("First argument must be an integer.")
        } else {
            4
        };

        let workers: Vec<_> = (0..num_procs).map(|_| cluster.fork()).collect();

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

        Promise::all(&promises).wait();

        println!("Master exiting...");
    }
}
