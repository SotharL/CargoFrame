//! Safe settings: what happens when sharing fails.
//!
//! The flow is: send with retries ([`retry`]); if that fails, hand the finished zip to the
//! fallback ladder ([`ladder`]): safe folder, then a folder the person picks, then CargoFrame's
//! own data folder. The zip is never deleted by the app.
//!
//! All decisions live here and all side effects go through traits, so the whole flow is tested
//! without touching the disk or the network.

pub mod error;
pub mod ladder;
pub mod naming;
pub mod retry;

pub use error::{FailureKind, SendError};
pub use ladder::{run_ladder, FallbackIo, LadderConfig, LadderReport, PickedFolder, Saved};
pub use retry::{run_with_retry, RetryPolicy, RetryResult};
