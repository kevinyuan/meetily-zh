//! Localised section headings for summary templates.
//!
//! Template section titles ("Summary", "Action Items", …) are written into the prompt
//! verbatim and the model copies them into the report, so a Chinese summary came back
//! with Chinese prose under English headings. Translating them here — rather than
//! asking the model to — keeps the headings exact and stable, which matters because
//! the section *instructions* refer to each section by title: if the heading and the
//! instruction disagree, the model loses the link between them.
//!
//! Titles not in the map pass through unchanged (custom templates), and the prompt
//! separately instructs the model to write headings in the target language, which
//! covers target languages we have no map for.

/// Built-in section titles, in the order they appear across the shipped templates.
const ZH_TITLES: &[(&str, &str)] = &[
    ("Summary", "摘要"),
    ("AI Session Summary", "AI 会话摘要"),
    ("Progress Summary", "进展摘要"),
    ("Key Decisions", "关键决策"),
    ("Action Items", "行动项"),
    ("Discussion Highlights", "讨论要点"),
    ("Next Steps", "后续步骤"),
    ("Blockers", "阻塞问题"),
    ("Risks & Concerns", "风险与顾虑"),
    ("Top Risks & Mitigations", "主要风险与应对"),
    ("Attendees", "参会人"),
    ("Attendance", "出席情况"),
    ("Notes", "备注"),
    ("Notes & Votes", "备注与表决"),
    ("Date", "日期"),
    ("Meeting Date & Time", "会议日期与时间"),
    ("Meeting Metadata", "会议信息"),
    ("Session Metadata", "会话信息"),
    ("Related Documents", "相关文档"),
    ("Audit Trail", "变更记录"),
    // Standup
    ("Yesterday", "昨天"),
    ("Today", "今天"),
    ("Sprint", "迭代"),
    // Retrospective
    ("Start Doing", "开始做"),
    ("Stop Doing", "停止做"),
    ("Continue Doing", "继续做"),
    // Project sync
    ("Milestones & Status", "里程碑与状态"),
    ("Agreed Deliverables", "已确认交付物"),
    // Sales / client call
    ("Client Goals & Success Criteria", "客户目标与成功标准"),
    ("Commercial Terms Discussed", "商务条款讨论"),
    // Clinical session (SOAP)
    ("Subjective (S)", "主观资料（S）"),
    ("Objective (O)", "客观资料（O）"),
    ("Assessment (A)", "评估（A）"),
    ("Plan (P)", "计划（P）"),
    ("Diagnoses (DSM/ICD)", "诊断（DSM/ICD）"),
    ("Medications", "用药"),
    ("Safety & Risk Management", "安全与风险管理"),
    ("Next Appointment", "下次预约"),
];

/// Translate a template section title into the target language.
///
/// `target_language` is the prompt-facing English language *name* ("Chinese",
/// "English", …), matching `processor::language_name_from_code`.
pub fn localize_section_title(title: &str, target_language: &str) -> String {
    if !is_chinese(target_language) {
        return title.to_string();
    }

    ZH_TITLES
        .iter()
        .find(|(en, _)| *en == title)
        .map(|(_, zh)| (*zh).to_string())
        .unwrap_or_else(|| title.to_string())
}

fn is_chinese(target_language: &str) -> bool {
    target_language == "Chinese" || target_language == "Traditional Chinese"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_titles_are_translated_for_chinese() {
        assert_eq!(localize_section_title("Summary", "Chinese"), "摘要");
        assert_eq!(localize_section_title("Action Items", "Chinese"), "行动项");
        assert_eq!(localize_section_title("Key Decisions", "Chinese"), "关键决策");
    }

    #[test]
    fn english_target_is_left_alone() {
        assert_eq!(localize_section_title("Summary", "English"), "Summary");
    }

    /// A custom template can use any heading; we must not drop or mangle it.
    #[test]
    fn unknown_titles_pass_through() {
        assert_eq!(
            localize_section_title("Bespoke Section", "Chinese"),
            "Bespoke Section"
        );
    }

    #[test]
    fn languages_without_a_map_keep_the_original() {
        assert_eq!(localize_section_title("Summary", "Japanese"), "Summary");
    }

    /// Every section title shipped in templates/*.json must have a translation —
    /// otherwise a Chinese summary silently grows an English heading.
    #[test]
    fn every_builtin_section_title_is_covered() {
        let templates: [&str; 6] = [
            include_str!("../../../templates/standard_meeting.json"),
            include_str!("../../../templates/daily_standup.json"),
            include_str!("../../../templates/retrospective.json"),
            include_str!("../../../templates/project_sync.json"),
            include_str!("../../../templates/sales_marketing_client_call.json"),
            include_str!("../../../templates/psychatric_session.json"),
        ];

        let mut missing: Vec<String> = Vec::new();
        for raw in templates {
            let value: serde_json::Value = serde_json::from_str(raw).expect("template is valid JSON");
            let Some(sections) = value.get("sections").and_then(|s| s.as_array()) else {
                continue;
            };
            for section in sections {
                let Some(title) = section.get("title").and_then(|t| t.as_str()) else {
                    continue;
                };
                if localize_section_title(title, "Chinese") == title {
                    missing.push(title.to_string());
                }
            }
        }

        assert!(
            missing.is_empty(),
            "built-in section titles with no Chinese translation: {:?}",
            missing
        );
    }
}
