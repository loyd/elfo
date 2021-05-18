// Let's build a simple application with three actor groups:
// * *producers* send some numbers to *aggregators*
// * *reporters* ask summaries from *aggregators*

//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
//              protocol
//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

// All actor crates depend on one or more protocols.
// Dependencies between actors should be avoided.
mod protocol {
    use elfo::prelude::*;

    // It's just a regular message.
    // `message` derives
    // * `Debug` for logging in dev env
    // * `Serialize` and `Deserialize` for dumping and comminication between nodes
    // * `Message` and `Request` to restrict contracts
    #[message]
    pub struct AddNum {
        pub group: u32,
        pub num: u32,
    }

    // Messages with specified `ret` are requests.
    #[message(ret = Summary)]
    pub struct Summarize {
        pub group_filter: Option<u32>, // `None` for all.
    }

    // Responses don't have to implement `Message`.
    #[message(part)] // derives `Debug`, `Clone`, `Serialize` and `Deserialize`.
    pub struct Summary {
        pub group: u32,
        pub sum: u32,
    }
}

//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
//              producer
//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

// An actor group with only one child so-called a singleton.
mod producer {
    use anyhow::bail;
    use elfo::{config::Secret, prelude::*};
    use serde::{Deserialize, Serialize};

    use crate::protocol::*;

    #[derive(Debug, Serialize, Deserialize)]
    struct Config {
        group_count: u32,
        item_count: u32,
        // Wrap credentials to hide them in logs and dumps.
        #[serde(default)]
        password: Secret<String>,
    }

    // It's a group factory. The module can have a lot of them with different
    // arguments, just like constructors.
    pub fn new() -> Schema {
        ActorGroup::new()
            .config::<Config>()
            .exec(move |ctx| async move {
                // Use `ctx.config()` to get an actual version of the config.
                let item_count = ctx.config().item_count;
                let group_count = ctx.config().group_count;

                // Send some numbers.
                for num in 0..item_count {
                    let group = num % group_count;

                    // `send().await` returns when the message is placed in a mailbox.
                    // ... or returns an error if all destinations are closed and cannot be
                    // restarted right now.
                    // Note that `elfo` logs warnings on its own and with restricted rate.
                    let _ = ctx.send(AddNum { group, num }).await;
                }

                // The supervisor will restart failed actors with back off mechanism.
                bail!("suicide");
            })
    }
}

//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
//              aggregator
//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

// A more actor group that has sharding.
mod aggregator {
    use elfo::{
        prelude::*,
        routers::{MapRouter, Outcome},
    };

    use crate::protocol::*;

    pub fn new() -> Schema {
        ActorGroup::new()
            // Routers are called on a sending side, potentially from many threads.
            // Usually, routers extract some sharding key from messages.
            //
            // See `MapRouter::with_state` for more complex routers with a state
            // (potentially depending on the config).
            .router(MapRouter::new(|envelope| {
                // `Envelope` is an abstract wrapper around message with some metadata.
                // Envelopes with known types are represented as `Envelope<T>`.
                //
                // It's not possible to mix different types in one `match`, thus
                // the special `msg!` macro should be used to beat it.
                // Reuse `match` syntax allows us to be compatible with `rustfmt`.
                msg!(match envelope {
                    // `Unicast` is for sending to only one specific actor.
                    // A new actor will be spawned if there is no actor for this key (`group`).
                    AddNum { group, .. } => Outcome::Unicast(*group),
                    // `Broadcast` is for sending to all already spawned actors.
                    Summarize { group_filter, .. } => group_filter
                        // Summarize only data of a specific group.
                        .map(Outcome::Unicast)
                        // Summarize all groups.
                        .unwrap_or(Outcome::Broadcast),
                    // Also there are other variants: `Multicast`, `Discard` and this one.
                    _ => Outcome::Default,
                })
            }))
            .exec(aggregator)
    }

    async fn aggregator(mut ctx: Context<(), u32>) {
        // Define some shard-specific state.
        let mut sum = 0;
        // Get a sharding key.
        let group = *ctx.key();

        // The main actor loop: receive a message, handle it, repeat.
        // Returns `None` and breaks the loop if actor's mailbox is closed
        // (usually when the system terminates).
        while let Some(envelope) = ctx.recv().await {
            msg!(match envelope {
                msg @ AddNum => {
                    sum += msg.num;
                }
                // It's a syntax for requests.
                // See more patterns in `elfo/tests/msg_macro.rs`.
                (Summarize, token) => {
                    // Use `token` to respond. The token cannot be used twice.
                    // If the token is dropped without responding,
                    // the sending side will get `RequestError::Ignored`.
                    let _ = ctx.respond(token, Summary { group, sum });
                }
            });
        }

        // Some work to perform a graceful termination.
    }
}

//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
//               reporter
//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

mod reporter {
    use std::time::Duration;

    use elfo::{
        messages::{ConfigUpdated, ValidateConfig},
        prelude::*,
        time::Interval,
    };
    use serde::{Deserialize, Serialize};

    use crate::protocol::*;

    #[derive(Debug, Serialize, Deserialize)]
    struct Config {
        #[serde(with = "humantime_serde")]
        interval: Duration,
    }

    // Sometimes it's useful to define private messages.
    #[message]
    struct TimerTick;

    pub fn new() -> Schema {
        ActorGroup::new().config::<Config>().exec(reporter)
    }

    async fn reporter(ctx: Context<Config>) {
        let interval = Interval::new(|| TimerTick);

        // It's possible to attach additional sources to handle everything the same way.
        let mut ctx = ctx.with(&interval);

        while let Some(envelope) = ctx.recv().await {
            // The setters of sources are cheap usually,
            // so it's possible to change it on each iteration.
            interval.set_period(ctx.config().interval);

            msg!(match envelope {
                (ValidateConfig { config, .. }, token) => {
                    // You can additionally validate a config against dynamic data.
                    // If all actors pass or ignore the validation step,
                    // configs are updated (`ConfigUpdated` event).
                    let _config = ctx.unpack_config(&config);
                    let _ = ctx.respond(token, Err("oops".into()));
                }
                ConfigUpdated => {
                    // Sometimes config updates require more complex actions,
                    // e.g. reopen connections. Do it here.
                }
                TimerTick => {
                    // `request(..).resolve().await` returns the result
                    // ... or with error, if something went wrong.
                    // In the future, `request(..).id().await` will be able to be used
                    // in order to get a response via the mailbox.
                    let req = Summarize { group_filter: None };
                    for summary in ctx.request(req).all().resolve().await {
                        tracing::info!(?summary, "summary");
                    }
                }
            });
        }
    }
}

//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
//               topology
//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

// Topology definition with actor groups and connections between them.
fn topology() -> elfo::Topology {
    let topology = elfo::Topology::empty();

    // Set up logging (based on the `tracing` crate).
    // `elfo` provides a logger actor group to support runtime control.
    // You can use `RUST_LOG=elfo` in dev to see messages between actors.
    // In the future, `elfo` will implement inexpensive dumping subsystem and tools
    // for regression testing & tracing.
    let logger = elfo::logger::init();

    // Define actor groups.
    let producers = topology.local("producers");
    let aggregators = topology.local("aggregators");
    let reporters = topology.local("reporters");
    let loggers = topology.local("system.loggers");
    // Check out https://elfo.rs/inspector for more details on a powerful
    // elfo inspection toolbox
    let inspectors = topology.local("system.inspectors");
    let configurers = topology.local("system.configurers").entrypoint();

    // Define links between actor groups.
    // Producers send raw data to aggregators.
    producers.route_all_to(&aggregators);
    // Reporters ask aggregators for a summary.
    reporters.route_all_to(&aggregators);

    // Mount specific implementations.
    producers.mount(producer::new());
    aggregators.mount(aggregator::new());
    reporters.mount(reporter::new());
    loggers.mount(logger);
    inspectors.mount(elfo::inspector::new(topology.clone()));

    // Actors can use `topology` as an extended service locator.
    // Usually it should be used for utilities only.
    let config_path = "elfo/examples/config.toml";
    configurers.mount(elfo::configurer::from_path(&topology, config_path));

    topology
}

//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
//                setup
//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

#[tokio::main]
async fn main() {
    elfo::start(topology()).await;
}
