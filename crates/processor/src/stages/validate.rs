//! Output validation before commit — Layer A recursive tileset validator (Phase 12).
//!
//! Replaces the previous weak "tileset.json exists + JSON parse" check.

use crate::protocol::{Emitter, Stage};
use crate::validator::{validate_and_write_report, ValidationReport};
use std::path::Path;

pub fn validate_tileset_dir(emitter: &Emitter, dir: &Path) -> Result<(), String> {
    emitter.stage(Stage::Validate, "Layer A recursive tileset validation");
    let report = validate_and_write_report(dir).map_err(|e| {
        format!("VALIDATION_FAILED: cannot write validation_internal.json: {e}")
    })?;

    emit_summary(emitter, &report);

    if !report.ok {
        let summary = report
            .first_error_summary()
            .unwrap_or_else(|| "validation failed".into());
        // Emit each error for JSONL consumers (cap to avoid flood).
        for issue in report
            .issues
            .iter()
            .filter(|i| i.severity == "error")
            .take(32)
        {
            emitter.error(&issue.code, &format!("{} @ {}", issue.message, issue.path));
        }
        return Err(format!("VALIDATION_FAILED: {summary}"));
    }

    for issue in report
        .issues
        .iter()
        .filter(|i| i.severity == "warning")
        .take(16)
    {
        emitter.warning(&issue.code, &format!("{} @ {}", issue.message, issue.path));
    }

    emitter.log(&format!(
        "[validate] Layer A OK tilesets={} contents={} external={} report={}",
        report.tileset_count,
        report.content_count,
        report.external_tileset_count,
        dir.join("validation_internal.json").display()
    ));
    Ok(())
}

fn emit_summary(emitter: &Emitter, report: &ValidationReport) {
    emitter.metric(
        "validation_internal",
        serde_json::json!({
            "ok": report.ok,
            "tilesetCount": report.tileset_count,
            "contentCount": report.content_count,
            "externalTilesetCount": report.external_tileset_count,
            "errorCount": report.error_count,
            "warningCount": report.warning_count,
        }),
    );
}

