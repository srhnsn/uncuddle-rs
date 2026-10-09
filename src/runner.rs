//! Orchestrate the analysis/fix pipeline without coupling it to CLI parsing
//! or rendering. All candidates are analyzed and preflighted before writing.
use crate::{analysis, config::Config, diagnostic::Diagnostic, discovery, fix};
use anyhow::Result;
use std::path::Path;

pub struct Report {
    pub files_checked: usize,
    pub files_fixed: usize,
    pub diagnostics: Vec<Diagnostic>,
    pub notices: Vec<String>,
}

impl Report {
    pub fn exit_code(&self) -> i32 {
        i32::from(!self.diagnostics.is_empty())
    }
}

struct FilePlan<'a> {
    file: &'a discovery::SourceFile,
    replacement: String,
    remaining: Vec<Diagnostic>,
}

pub fn run(
    options: &discovery::Options,
    fix_mode: bool,
    config_path: Option<&Path>,
) -> Result<Report> {
    let targets = discovery::load_targets(options)?;
    let config = Config::load(&targets.root, config_path)?;
    let project = discovery::scan(targets, &config)?;
    let mut plans = Vec::new();

    for file in &project.files {
        let diagnostics =
            analysis::analyze_with_edition(&file.path, &file.source, &file.edition, &config)?;
        let (replacement, remaining) = if fix_mode {
            let replacement = fix::apply(&file.source, &diagnostics)?;
            let remaining =
                analysis::analyze_with_edition(&file.path, &replacement, &file.edition, &config)?;

            (replacement, remaining)
        } else {
            (file.source.clone(), diagnostics)
        };
        plans.push(FilePlan {
            file,
            replacement,
            remaining,
        });
    }

    if fix_mode {
        for plan in &plans {
            if plan.file.source != plan.replacement {
                fix::verify_writable(&plan.file.path)?;
                fix::verify_unchanged(&plan.file.path, &plan.file.source)?;
            }
        }
    }

    let mut report = Report {
        files_checked: project.files.len(),
        files_fixed: 0,
        diagnostics: Vec::new(),
        notices: project.notices,
    };

    for plan in plans {
        if fix_mode {
            fix::write_if_unchanged(&plan.file.path, &plan.file.source, &plan.replacement)?;
            report.files_fixed += usize::from(plan.file.source != plan.replacement);
        }

        report.diagnostics.extend(plan.remaining);
    }

    Ok(report)
}
