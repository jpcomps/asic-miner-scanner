use std::future::Future;
use std::sync::OnceLock;

fn global_runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("asic-miner-scanner-async")
            .build()
            .expect("failed to initialize global async runtime")
    })
}

pub fn spawn<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    let _ = global_runtime().spawn(future);
}
