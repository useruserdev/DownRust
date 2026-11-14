// SPDX-License-Identifier: GPL-3.0-or-later
//! Transfer engine internals.
pub(crate) mod attempt;
pub(crate) mod checksum;
pub(crate) mod coordinator;
pub(crate) mod file;
pub(crate) mod limiter;
pub(crate) mod live;
pub(crate) mod media;
pub(crate) mod plan;
pub(crate) mod retry;
pub(crate) mod scan;
pub(crate) mod segments;
pub(crate) mod speed;
pub(crate) mod task;
pub(crate) mod worker;
pub(crate) mod worker_stream;
pub(crate) mod writer;
