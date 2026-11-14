// SPDX-License-Identifier: GPL-3.0-or-later
//! Commands sent to the manager actor, the dispatch of each command, and the helper that sends
//! one command and waits for its reply.
use tokio::sync::{mpsc::UnboundedSender, oneshot};

use super::actor::ManagerActor;
use crate::engine::task::TaskOutcome;
use crate::error::{Error, Result};
use crate::model::{AddRequest, ChecksumState, DownloadId, DownloadRecord, ProbeInfo, QueueMove};
use crate::settings::EngineSettings;

pub(super) type Reply<T> = oneshot::Sender<Result<T>>;

/// An add request whose file name and probe result are already settled.
pub(super) struct ResolvedAdd {
    pub req: AddRequest,
    pub file_name: String,
    pub probe: Option<ProbeInfo>,
}

pub(super) enum Cmd {
    Add {
        req: Box<ResolvedAdd>,
        reply: Reply<DownloadRecord>,
    },
    List {
        reply: Reply<Vec<DownloadRecord>>,
    },
    Get {
        id: DownloadId,
        reply: Reply<DownloadRecord>,
    },
    Pause {
        id: DownloadId,
        reply: Reply<()>,
    },
    Resume {
        id: DownloadId,
        reply: Reply<()>,
    },
    Restart {
        id: DownloadId,
        reply: Reply<()>,
    },
    Remove {
        id: DownloadId,
        delete_file: bool,
        reply: Reply<()>,
    },
    PauseAll {
        reply: Reply<()>,
    },
    ResumeAll {
        reply: Reply<()>,
    },
    SetConnections {
        id: DownloadId,
        connections: u8,
        reply: Reply<()>,
    },
    SetSpeedLimit {
        id: DownloadId,
        limit: Option<u64>,
        reply: Reply<()>,
    },
    UpdateSettings {
        settings: Box<EngineSettings>,
        reply: Reply<()>,
    },
    MoveInQueue {
        id: DownloadId,
        to: QueueMove,
        reply: Reply<()>,
    },
    RecordChecksum {
        id: DownloadId,
        expected: String,
        state: ChecksumState,
        reply: Reply<()>,
    },
    UpdateAddress {
        id: DownloadId,
        url: String,
        probe: Box<ProbeInfo>,
        reply: Reply<DownloadRecord>,
    },
    Shutdown {
        reply: oneshot::Sender<()>,
    },
    TaskFinished {
        id: DownloadId,
        outcome: TaskOutcome,
    },
    Reschedule,
}

/// Sends a command built around a fresh reply channel and waits for the answer.
/// A stopped actor gives `ManagerStopped`.
pub(super) async fn request<T>(
    tx: &UnboundedSender<Cmd>,
    build: impl FnOnce(Reply<T>) -> Cmd,
) -> Result<T> {
    let (reply, answer) = oneshot::channel();
    tx.send(build(reply)).map_err(|_| Error::ManagerStopped)?;
    answer.await.map_err(|_| Error::ManagerStopped)?
}

impl ManagerActor {
    /// Runs one command. Returns true when the actor must stop.
    pub(super) fn handle(&mut self, cmd: Cmd) -> bool {
        match cmd {
            Cmd::Add { req, reply } => reply_with(reply, self.add(*req)),
            Cmd::List { reply } => reply_with(reply, self.list()),
            Cmd::Get { id, reply } => reply_with(reply, self.get(id)),
            Cmd::Pause { id, reply } => reply_with(reply, self.pause(id)),
            Cmd::Resume { id, reply } => reply_with(reply, self.resume(id)),
            Cmd::Restart { id, reply } => reply_with(reply, self.restart(id)),
            Cmd::Remove {
                id,
                delete_file,
                reply,
            } => reply_with(reply, self.remove(id, delete_file)),
            Cmd::PauseAll { reply } => reply_with(reply, self.pause_all()),
            Cmd::ResumeAll { reply } => reply_with(reply, self.resume_all()),
            Cmd::SetConnections {
                id,
                connections,
                reply,
            } => reply_with(reply, self.set_connections(id, connections)),
            Cmd::SetSpeedLimit { id, limit, reply } => {
                reply_with(reply, self.set_speed_limit(id, limit))
            }
            Cmd::UpdateSettings { settings, reply } => {
                reply_with(reply, self.update_settings(*settings))
            }
            Cmd::MoveInQueue { id, to, reply } => reply_with(reply, self.move_in_queue(id, to)),
            Cmd::RecordChecksum {
                id,
                expected,
                state,
                reply,
            } => reply_with(reply, self.record_checksum(id, &expected, state)),
            Cmd::UpdateAddress {
                id,
                url,
                probe,
                reply,
            } => reply_with(reply, self.update_address(id, &url, &probe)),
            Cmd::Shutdown { reply } => {
                self.shutting_down = true;
                self.shutdown_reply = Some(reply);
                for task in self.active.values() {
                    task.cancel.cancel();
                }
                self.stop_if_idle()
            }
            Cmd::TaskFinished { id, outcome } => {
                if let Err(err) = self.finish_task(id, outcome) {
                    tracing::error!("cannot record the end of download {id}: {err}");
                }
                self.stop_if_idle()
            }
            Cmd::Reschedule => {
                self.schedule();
                false
            }
        }
    }

    /// Replies to shutdown once no transfer is running. Returns true when the actor must stop.
    fn stop_if_idle(&mut self) -> bool {
        if !(self.shutting_down && self.active.is_empty()) {
            return false;
        }
        if let Some(reply) = self.shutdown_reply.take() {
            let _ = reply.send(());
        }
        true
    }
}

/// Sends a handler result to its caller. Always returns false: the actor keeps running.
fn reply_with<T>(reply: Reply<T>, result: Result<T>) -> bool {
    let _ = reply.send(result);
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_with_delivers_the_result() {
        let (reply, answer) = oneshot::channel();
        assert!(!reply_with(reply, Ok::<u32, Error>(7)));
        assert_eq!(answer.blocking_recv().expect("reply").expect("ok"), 7);
    }
}
