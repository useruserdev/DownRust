// SPDX-License-Identifier: GPL-3.0-or-later
//! Positioned file writer that runs on a blocking thread.
use std::fs::File;
use std::io;
use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use super::file::write_all_at;
use super::segments::{SegmentSlot, SegmentTable};

/// Command sent to the writer thread.
pub(crate) enum WriteCmd {
    Data {
        slot: usize,
        offset: u64,
        data: Bytes,
    },
    Checkpoint {
        reply: oneshot::Sender<Vec<SegmentSlot>>,
    },
}

pub(crate) const WRITE_QUEUE: usize = 64;

pub(crate) struct WriterHandle {
    pub tx: mpsc::Sender<WriteCmd>,
    pub join: JoinHandle<io::Result<File>>,
}

/// Starts the writer. It ends when every sender is dropped, then syncs the file and returns it.
pub(crate) fn spawn_writer(file: File, table: Arc<SegmentTable>) -> WriterHandle {
    let (tx, mut rx) = mpsc::channel(WRITE_QUEUE);
    let join = tokio::task::spawn_blocking(move || -> io::Result<File> {
        while let Some(cmd) = rx.blocking_recv() {
            match cmd {
                WriteCmd::Data { slot, offset, data } => {
                    write_all_at(&file, &data, offset)?;
                    table.commit(slot, data.len() as u64);
                }
                WriteCmd::Checkpoint { reply } => {
                    file.sync_data()?;
                    let _ = reply.send(table.snapshot());
                }
            }
        }
        file.sync_all()?;
        Ok(file)
    });
    WriterHandle { tx, join }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::file::open_part_file;

    fn slot(start: u64, end: u64) -> SegmentSlot {
        SegmentSlot {
            start,
            end,
            fetched: 0,
            written: 0,
            active: true,
        }
    }

    #[tokio::test]
    async fn writes_land_at_offsets_and_checkpoint_returns_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.part");
        let file = open_part_file(&path, true).unwrap();
        let table = Arc::new(SegmentTable::new(vec![slot(0, 5), slot(5, 10)]));
        table.accept(0, 5);
        table.accept(1, 5);
        let w = spawn_writer(file, table.clone());
        w.tx.send(WriteCmd::Data {
            slot: 1,
            offset: 5,
            data: Bytes::from_static(b"fghij"),
        })
        .await
        .unwrap();
        w.tx.send(WriteCmd::Data {
            slot: 0,
            offset: 0,
            data: Bytes::from_static(b"abcde"),
        })
        .await
        .unwrap();
        let (reply, rx) = oneshot::channel();
        w.tx.send(WriteCmd::Checkpoint { reply }).await.unwrap();
        let snap = rx.await.unwrap();
        assert_eq!(snap[0].written, 5);
        assert_eq!(snap[1].written, 5);
        let WriterHandle { tx, join } = w;
        drop(tx);
        let file = join.await.unwrap().unwrap();
        drop(file);
        assert_eq!(std::fs::read(&path).unwrap(), b"abcdefghij");
    }
}
