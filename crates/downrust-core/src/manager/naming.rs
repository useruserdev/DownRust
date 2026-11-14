// SPDX-License-Identifier: GPL-3.0-or-later
//! Save folder and file name selection for new downloads.
use std::cell::RefCell;
use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::model::{Category, DownloadKind, TargetSuggestion, PART_EXTENSION};
use crate::settings::EngineSettings;
use crate::store::Store;
use crate::util::filename::{sanitize_file_name, unique_file_name};

/// Picks the category, the folder and a unique file name for a new download.
/// A non-empty `save_dir_override` must be an absolute path and replaces the category folder.
pub(super) fn resolve_target(
    store: &Store,
    settings: &EngineSettings,
    desired: &str,
    save_dir_override: Option<&str>,
) -> Result<TargetSuggestion> {
    let name = sanitize_file_name(desired);
    let category = Category::from_file_name(&name);
    let dir = match save_dir_override
        .map(str::trim)
        .filter(|dir| !dir.is_empty())
    {
        Some(path) => {
            let dir = PathBuf::from(path);
            if !dir.is_absolute() {
                return Err(Error::InvalidInput(format!(
                    "save folder must be an absolute path: {path}"
                )));
            }
            dir
        }
        None => settings.dir_for(category),
    };
    let dir_text = dir.to_string_lossy().into_owned();
    let store_error: RefCell<Option<Error>> = RefCell::new(None);
    let file_name = unique_file_name(&name, |candidate| {
        if dir.join(candidate).exists() || dir.join(format!("{candidate}{PART_EXTENSION}")).exists()
        {
            return true;
        }
        match store.is_name_taken(&dir_text, candidate) {
            Ok(taken) => taken,
            Err(err) => {
                store_error.borrow_mut().get_or_insert(err);
                true
            }
        }
    });
    if let Some(err) = store_error.into_inner() {
        return Err(err);
    }
    Ok(TargetSuggestion {
        category,
        save_dir: dir_text,
        file_name,
    })
}

/// File name of an HLS or DASH download that has no title: the last part of the address without
/// its extension, plus the extension of the output (`.mp4`, or `.ts` for an unconverted HLS file).
pub(super) fn media_file_name(url: &str, kind: DownloadKind, convert_to_mp4: bool) -> String {
    let path = url.split(['?', '#']).next().unwrap_or_default();
    let last = path
        .rsplit('/')
        .find(|part| !part.is_empty() && !part.contains(':'))
        .unwrap_or_default();
    let stem = last.rsplit_once('.').map_or(last, |(stem, _)| stem);
    let stem = if stem.trim().is_empty() {
        "video"
    } else {
        stem
    };
    let extension = if kind == DownloadKind::Dash || convert_to_mp4 {
        "mp4"
    } else {
        "ts"
    };
    format!("{stem}.{extension}")
}

/// A title given for an HLS or DASH download, with the output extension added when the title has
/// no video extension, so the download is sorted into the Video category.
pub(super) fn media_name_with_extension(
    name: &str,
    kind: DownloadKind,
    convert_to_mp4: bool,
) -> String {
    if Category::from_file_name(name) == Category::Video {
        return name.to_owned();
    }
    let extension = if kind == DownloadKind::Dash || convert_to_mp4 {
        "mp4"
    } else {
        "ts"
    };
    format!("{}.{extension}", name.trim_end_matches('.'))
}

#[cfg(test)]
mod tests {
    #[test]
    fn media_titles_get_a_video_extension() {
        use super::media_name_with_extension as name;
        assert_eq!(name("Lecture 7", DownloadKind::Hls, true), "Lecture 7.mp4");
        assert_eq!(name("Lecture 7", DownloadKind::Hls, false), "Lecture 7.ts");
        assert_eq!(name("Talk", DownloadKind::Dash, false), "Talk.mp4");
        assert_eq!(name("clip.mp4", DownloadKind::Hls, false), "clip.mp4");
        assert_eq!(
            Category::from_file_name(&name("Lecture 7", DownloadKind::Hls, true)),
            Category::Video
        );
    }

    use super::*;
    use crate::model::{DownloadStatus, NewDownload};

    #[test]
    fn media_names_use_the_address_and_the_output_extension() {
        assert_eq!(
            media_file_name(
                "https://a.test/live/index.m3u8?x=1",
                DownloadKind::Hls,
                true
            ),
            "index.mp4"
        );
        assert_eq!(
            media_file_name("https://a.test/live/index.m3u8", DownloadKind::Hls, false),
            "index.ts"
        );
        assert_eq!(
            media_file_name("https://a.test/", DownloadKind::Dash, false),
            "a.mp4"
        );
    }

    fn row(file_name: &str) -> NewDownload {
        NewDownload {
            url: "https://example.com/file".into(),
            final_url: None,
            referrer: None,
            file_name: file_name.into(),
            save_dir: "C:/unused".into(),
            category: Category::Other,
            status: DownloadStatus::Paused,
            total_size: None,
            resumable: false,
            connections: 1,
            speed_limit: None,
            etag: None,
            last_modified: None,
            content_type: None,
            headers: Vec::new(),
            queue_position: 1,
            created_at: 0,
            expected_sha256: None,
            kind: crate::model::DownloadKind::File,
            mirrors: Vec::new(),
            options: None,
        }
    }

    #[test]
    fn skips_names_already_in_the_store_case_insensitively() {
        let store = Store::open_in_memory().expect("store");
        let dir = tempfile::tempdir().expect("tempdir");
        let dir_text = dir.path().to_string_lossy().into_owned();
        let mut existing = row("REPORT.pdf");
        existing.save_dir = dir_text.clone();
        store.insert_download(&existing).expect("insert");
        let settings = EngineSettings::default();
        let target = resolve_target(&store, &settings, "report.pdf", Some(&dir_text)).expect("ok");
        assert_eq!(target.file_name, "report (1).pdf");
        assert_eq!(target.save_dir, dir_text);
    }

    #[test]
    fn skips_files_on_disk_and_part_files() {
        let store = Store::open_in_memory().expect("store");
        let dir = tempfile::tempdir().expect("tempdir");
        let dir_text = dir.path().to_string_lossy().into_owned();
        std::fs::write(dir.path().join("a.zip"), b"x").expect("write");
        std::fs::write(dir.path().join("b.zip.downrust"), b"x").expect("write");
        let settings = EngineSettings::default();
        let a = resolve_target(&store, &settings, "a.zip", Some(&dir_text)).expect("ok");
        let b = resolve_target(&store, &settings, "b.zip", Some(&dir_text)).expect("ok");
        assert_eq!(a.file_name, "a (1).zip");
        assert_eq!(b.file_name, "b (1).zip");
    }

    #[test]
    fn sanitizes_the_desired_name() {
        let store = Store::open_in_memory().expect("store");
        let dir = tempfile::tempdir().expect("tempdir");
        let dir_text = dir.path().to_string_lossy().into_owned();
        let settings = EngineSettings::default();
        let target = resolve_target(&store, &settings, "a:b?.txt", Some(&dir_text)).expect("ok");
        assert_eq!(target.file_name, "a_b_.txt");
    }

    #[test]
    fn category_folder_is_used_when_auto_sort_is_on() {
        let store = Store::open_in_memory().expect("store");
        let root = tempfile::tempdir().expect("tempdir");
        let mut settings = EngineSettings {
            default_dir: root.path().to_string_lossy().into_owned(),
            auto_sort_by_category: true,
            ..EngineSettings::default()
        };
        settings.fill_missing_dirs(root.path());
        let video = resolve_target(&store, &settings, "movie.mp4", None).expect("ok");
        assert_eq!(video.category, Category::Video);
        assert_eq!(
            video.save_dir,
            root.path().join("Video").to_string_lossy().into_owned()
        );
        let other = resolve_target(&store, &settings, "notes.xyz", None).expect("ok");
        assert_eq!(other.category, Category::Other);
        assert_eq!(other.save_dir, settings.default_dir);
    }

    #[test]
    fn default_folder_is_used_when_auto_sort_is_off() {
        let store = Store::open_in_memory().expect("store");
        let root = tempfile::tempdir().expect("tempdir");
        let mut settings = EngineSettings {
            default_dir: root.path().to_string_lossy().into_owned(),
            auto_sort_by_category: false,
            ..EngineSettings::default()
        };
        settings.fill_missing_dirs(root.path());
        let target = resolve_target(&store, &settings, "movie.mp4", None).expect("ok");
        assert_eq!(target.save_dir, settings.default_dir);
    }

    #[test]
    fn relative_override_is_rejected() {
        let store = Store::open_in_memory().expect("store");
        let settings = EngineSettings::default();
        let result = resolve_target(&store, &settings, "a.bin", Some("relative/dir"));
        assert!(matches!(result, Err(Error::InvalidInput(_))));
    }
}
