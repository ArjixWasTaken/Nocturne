//! Bridge between the glib main loop (UI) and the tokio runtime (I/O).
use std::{future::Future, sync::OnceLock};
use tokio::runtime::Runtime;

pub fn rt() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| Runtime::new().expect("failed to start tokio runtime"))
}

/// Run `fut` on tokio and await its output from any executor (typically the glib main context).
pub async fn run<F>(fut: F) -> F::Output
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    rt().spawn(fut).await.expect("tokio task panicked")
}

/// Spawn a future on the glib main context (may touch widgets).
pub fn spawn_local<F: Future<Output = ()> + 'static>(fut: F) {
    gtk::glib::spawn_future_local(fut);
}
