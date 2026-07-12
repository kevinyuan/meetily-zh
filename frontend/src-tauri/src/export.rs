//! Export meetings to Markdown files.
//!
//! Transcripts and summaries live in SQLite, which is right for the app's access
//! patterns (per-meeting queries, search, in-place summary updates) but useless if you
//! want your notes in a folder, in Git, or synced to iCloud. Exporting sidesteps that:
//! the database stays the source of truth, and the files are a plain-text copy you own.

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_dialog::DialogExt;

use crate::database::repositories::{meeting::MeetingsRepository, summary::SummaryProcessesRepository};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct ExportResult {
    /// Folder the files were written to.
    pub folder: String,
    pub exported: usize,
    /// Meetings that produced no file (no transcript and no summary).
    pub skipped: usize,
}

/// Section headings, in the user's language. This is a Chinese-first build, so an
/// unknown/absent locale gets Chinese rather than English.
struct Headings {
    date: &'static str,
    summary: &'static str,
    transcript: &'static str,
    no_summary: &'static str,
}

fn headings_for(language: Option<&str>) -> Headings {
    let is_english = language.is_some_and(|l| l.starts_with("en"));
    if is_english {
        Headings {
            date: "Date",
            summary: "Summary",
            transcript: "Transcript",
            no_summary: "_No summary generated._",
        }
    } else {
        Headings {
            date: "日期",
            summary: "摘要",
            transcript: "转录",
            no_summary: "_尚未生成摘要。_",
        }
    }
}

/// Make a title safe to use as a filename, without mangling CJK.
///
/// Only the characters that actually break filesystems are replaced — stripping
/// everything non-ASCII would reduce a Chinese meeting title to an empty string.
fn safe_filename(title: &str) -> String {
    const FORBIDDEN: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

    let cleaned: String = title
        .chars()
        .map(|c| {
            if FORBIDDEN.contains(&c) || c.is_control() {
                '-'
            } else {
                c
            }
        })
        .collect();

    let cleaned = cleaned.trim().trim_matches('.').trim();

    if cleaned.is_empty() {
        "untitled".to_string()
    } else {
        // Leave room for the date prefix and extension within the usual 255-byte limit.
        cleaned.chars().take(120).collect()
    }
}

/// Pull the summary markdown out of the stored process row.
///
/// `summary_processes.result` is a JSON blob whose `markdown` field holds the rendered
/// summary (see summary/service.rs).
fn summary_markdown(result_json: Option<&str>) -> Option<String> {
    let raw = result_json?;
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let markdown = value.get("markdown")?.as_str()?.trim();
    (!markdown.is_empty()).then(|| markdown.to_string())
}

fn build_markdown(
    title: &str,
    created_at: &str,
    summary: Option<&str>,
    transcripts: &[(String, String)],
    h: &Headings,
) -> String {
    let mut out = String::new();

    out.push_str(&format!("# {}\n\n", title));
    out.push_str(&format!("- {}: {}\n\n", h.date, created_at));

    out.push_str(&format!("## {}\n\n", h.summary));
    match summary {
        Some(md) => {
            out.push_str(md);
            out.push_str("\n\n");
        }
        None => out.push_str(&format!("{}\n\n", h.no_summary)),
    }

    out.push_str(&format!("## {}\n\n", h.transcript));
    for (timestamp, text) in transcripts {
        if text.trim().is_empty() {
            continue;
        }
        out.push_str(&format!("**[{}]** {}\n\n", timestamp, text.trim()));
    }

    out
}

/// Export meetings as Markdown. `meeting_ids = None` exports everything.
///
/// Prompts for a destination folder; returns Ok(None-equivalent) semantics via an error
/// only on real failure — a cancelled picker is reported as such so the UI can stay quiet.
#[tauri::command]
pub async fn export_meetings_markdown<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    meeting_ids: Option<Vec<String>>,
    language: Option<String>,
) -> Result<ExportResult, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |folder| {
        let _ = tx.send(folder);
    });

    let folder = rx
        .await
        .map_err(|_| "Folder picker closed unexpectedly".to_string())?
        .ok_or_else(|| "cancelled".to_string())?;

    let folder = folder
        .into_path()
        .map_err(|e| format!("Invalid destination folder: {}", e))?;

    let pool = state.db_manager.pool();
    let h = headings_for(language.as_deref());

    // Resolve which meetings to export.
    let ids: Vec<String> = match meeting_ids {
        Some(ids) => ids,
        None => MeetingsRepository::get_meetings(pool)
            .await
            .map_err(|e| format!("Failed to list meetings: {}", e))?
            .into_iter()
            .map(|m| m.id)
            .collect(),
    };

    let mut exported = 0usize;
    let mut skipped = 0usize;

    for id in ids {
        let details = match MeetingsRepository::get_meeting(pool, &id).await {
            Ok(Some(d)) => d,
            Ok(None) => {
                skipped += 1;
                continue;
            }
            Err(e) => {
                log::warn!("Skipping meeting {} — failed to load: {}", id, e);
                skipped += 1;
                continue;
            }
        };

        let summary = SummaryProcessesRepository::get_summary_data(pool, &id)
            .await
            .ok()
            .flatten()
            .and_then(|p| summary_markdown(p.result.as_deref()));

        let transcripts: Vec<(String, String)> = details
            .transcripts
            .iter()
            .map(|t| (t.timestamp.clone(), t.text.clone()))
            .collect();

        // A meeting with neither a transcript nor a summary has nothing to write.
        if transcripts.is_empty() && summary.is_none() {
            skipped += 1;
            continue;
        }

        let created_date = details.created_at.split('T').next().unwrap_or("").to_string();
        let filename = format!(
            "{}_{}.md",
            if created_date.is_empty() {
                "undated".to_string()
            } else {
                created_date
            },
            safe_filename(&details.title)
        );

        let markdown = build_markdown(
            &details.title,
            &details.created_at,
            summary.as_deref(),
            &transcripts,
            &h,
        );

        let path = folder.join(&filename);
        match std::fs::write(&path, markdown) {
            Ok(()) => exported += 1,
            Err(e) => {
                log::error!("Failed to write {}: {}", path.display(), e);
                skipped += 1;
            }
        }
    }

    log::info!(
        "Exported {} meeting(s) to {} ({} skipped)",
        exported,
        folder.display(),
        skipped
    );

    Ok(ExportResult {
        folder: folder.to_string_lossy().to_string(),
        exported,
        skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cjk_titles_survive_filename_sanitisation() {
        // Stripping non-ASCII would leave a Chinese meeting with an empty filename.
        assert_eq!(safe_filename("产品评审会议"), "产品评审会议");
        assert_eq!(safe_filename("Q3 规划/复盘"), "Q3 规划-复盘");
    }

    #[test]
    fn path_breaking_characters_are_replaced() {
        assert_eq!(safe_filename("a/b\\c:d*e?f\"g<h>i|j"), "a-b-c-d-e-f-g-h-i-j");
        assert_eq!(safe_filename("   "), "untitled");
        assert_eq!(safe_filename(""), "untitled");
    }

    #[test]
    fn summary_markdown_is_read_from_the_result_json() {
        assert_eq!(
            summary_markdown(Some("{\"markdown\":\"# Notes\\nBody\"}")).as_deref(),
            Some("# Notes\nBody")
        );
        assert_eq!(summary_markdown(Some(r#"{"markdown":"   "}"#)), None);
        assert_eq!(summary_markdown(Some("not json")), None);
        assert_eq!(summary_markdown(None), None);
    }

    #[test]
    fn markdown_has_summary_and_transcript_sections() {
        let h = headings_for(Some("zh-Hans"));
        let md = build_markdown(
            "周会",
            "2026-07-12T10:00:00Z",
            Some("要点若干"),
            &[("00:01".to_string(), "大家好".to_string())],
            &h,
        );
        assert!(md.starts_with("# 周会\n"));
        assert!(md.contains("## 摘要"));
        assert!(md.contains("要点若干"));
        assert!(md.contains("## 转录"));
        assert!(md.contains("**[00:01]** 大家好"));
    }

    #[test]
    fn a_meeting_without_a_summary_says_so_rather_than_leaving_a_hole() {
        let h = headings_for(Some("en"));
        let md = build_markdown("Standup", "2026-07-12T10:00:00Z", None, &[], &h);
        assert!(md.contains("_No summary generated._"));
    }

    #[test]
    fn unknown_locale_defaults_to_chinese_not_english() {
        assert_eq!(headings_for(None).summary, "摘要");
        assert_eq!(headings_for(Some("en-US")).summary, "Summary");
    }
}
