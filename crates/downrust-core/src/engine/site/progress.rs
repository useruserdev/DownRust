// SPDX-License-Identifier: GPL-3.0-or-later
//! Reads the progress lines printed by yt-dlp with the DownRust progress template.

/// Start of every progress line. The template is `download:DRP <downloaded> <total> <estimate> <speed>`.
pub(crate) const PREFIX: &str = "DRP ";

/// One progress report for the part that yt-dlp is downloading now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sample {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub estimate: Option<u64>,
}

/// Reads one progress line. Returns `None` for other lines and for a line without a byte count.
pub(crate) fn parse_line(line: &str) -> Option<Sample> {
    let rest = line.trim().strip_prefix(PREFIX)?;
    let mut fields = rest.split_whitespace();
    let downloaded = number(fields.next()?)?;
    let total = fields.next().and_then(number);
    let estimate = fields.next().and_then(number);
    Some(Sample {
        downloaded,
        total,
        estimate,
    })
}

/// A byte count. yt-dlp prints `NA` when it does not know a value.
fn number(text: &str) -> Option<u64> {
    if text == "NA" {
        return None;
    }
    let value: f64 = text.parse().ok()?;
    (value.is_finite() && value >= 0.0).then_some(value as u64)
}

/// Adds up the parts of one video. yt-dlp downloads the video and the audio one after the other,
/// and the byte count starts again from zero for each part.
#[derive(Debug)]
pub(crate) struct PartTracker {
    finished_done: u64,
    /// Sum of the sizes of the finished parts, `None` once one of them had no known size.
    finished_total: Option<u64>,
    current_done: u64,
    current_total: Option<u64>,
    started: bool,
}

impl Default for PartTracker {
    fn default() -> Self {
        Self {
            finished_done: 0,
            finished_total: Some(0),
            current_done: 0,
            current_total: None,
            started: false,
        }
    }
}

impl PartTracker {
    /// Takes one progress report. A smaller byte count starts a new part.
    pub fn push(&mut self, sample: Sample) {
        if self.started && sample.downloaded < self.current_done {
            self.finished_done += self.current_done;
            self.finished_total = match (self.finished_total, self.current_total) {
                (Some(sum), Some(size)) => Some(sum + size),
                _ => None,
            };
            self.current_done = 0;
            self.current_total = None;
        }
        self.started = true;
        self.current_done = sample.downloaded;
        self.current_total = sample.total.or(sample.estimate);
    }

    /// Bytes downloaded over all parts.
    pub fn done(&self) -> u64 {
        self.finished_done + self.current_done
    }

    /// Expected bytes over all parts, `None` while a size is not known.
    pub fn total(&self) -> Option<u64> {
        match (self.finished_total, self.current_total) {
            (Some(sum), Some(size)) => Some(sum + size),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_lines_are_read() {
        assert_eq!(
            parse_line("DRP 1024 4096 NA 2048.5"),
            Some(Sample {
                downloaded: 1024,
                total: Some(4096),
                estimate: None,
            })
        );
        assert_eq!(
            parse_line("DRP 10 NA 9000.0 NA"),
            Some(Sample {
                downloaded: 10,
                total: None,
                estimate: Some(9000),
            })
        );
        assert_eq!(parse_line("DRP NA 100 100 1"), None);
        assert_eq!(parse_line("[download] 50%"), None);
    }

    #[test]
    fn parts_are_added_up() {
        let mut tracker = PartTracker::default();
        let sample = |downloaded, total| Sample {
            downloaded,
            total: Some(total),
            estimate: None,
        };
        tracker.push(sample(0, 1000));
        tracker.push(sample(600, 1000));
        assert_eq!((tracker.done(), tracker.total()), (600, Some(1000)));
        tracker.push(sample(1000, 1000));
        tracker.push(sample(0, 300));
        assert_eq!((tracker.done(), tracker.total()), (1000, Some(1300)));
        tracker.push(sample(150, 300));
        assert_eq!((tracker.done(), tracker.total()), (1150, Some(1300)));
    }

    #[test]
    fn unknown_part_size_hides_the_total() {
        let mut tracker = PartTracker::default();
        tracker.push(Sample {
            downloaded: 50,
            total: None,
            estimate: None,
        });
        assert_eq!(tracker.total(), None);
        tracker.push(Sample {
            downloaded: 0,
            total: Some(200),
            estimate: None,
        });
        assert_eq!(tracker.total(), None);
        assert_eq!(tracker.done(), 50);
    }
}
