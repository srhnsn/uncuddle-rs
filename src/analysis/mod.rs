use crate::config::Config;
use crate::diagnostic::Diagnostic;
use attributes::{expr_attrs, skip};
use std::collections::BTreeMap;
use std::path::Path;
use syn::Expr;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use trivia::Gap;

mod attributes;
mod lines;
mod rules;
mod statements;
mod trivia;

/// Analyze source without compiling or invoking rustfmt. Macro contents are
/// opaque; edits are only proposed at boundaries between AST siblings.
pub fn analyze(path: &Path, source: &str, config: &Config) -> Result<Vec<Diagnostic>, syn::Error> {
    let ast = crate::parse(source)?;
    let mut analyzer = Analyzer {
        path,
        source,
        config,
        diagnostics: BTreeMap::new(),
        lines: lines::LineIndex::new(source),
    };
    analyzer.visit_file(&ast);
    Ok(analyzer.diagnostics.into_values().collect())
}

struct Analyzer<'a> {
    path: &'a Path,
    source: &'a str,
    config: &'a Config,
    diagnostics: BTreeMap<usize, Diagnostic>,
    lines: lines::LineIndex,
}

impl Analyzer<'_> {
    fn boundary(
        &mut self,
        previous: proc_macro2::Span,
        next: proc_macro2::Span,
        rule: &'static str,
    ) {
        if !self.config.enabled(rule) {
            return;
        }
        let end = previous.byte_range().end;
        let start = next.byte_range().start;
        if end > start || start > self.source.len() {
            return;
        }
        let Some(gap) = Gap::scan(self.source, end, start) else {
            self.report(start, rule, None);
            return;
        };
        if !gap.has_blank {
            self.report(start, rule, gap.insert_at);
        }
    }

    fn report(&mut self, start: usize, rule: &'static str, insert_at: Option<usize>) {
        let (line, column) = self.lines.location(self.source, start);
        self.diagnostics.entry(start).or_insert_with(|| Diagnostic {
            path: self.path.to_owned(),
            line,
            column,
            rule,
            message: "expected a blank line before this statement".into(),
            insert_at,
        });
    }

    fn block(&mut self, block: &syn::Block) {
        for (index, pair) in block.stmts.windows(2).enumerate() {
            let (previous, next) = (&pair[0], &pair[1]);
            let rule = rules::for_boundary(
                previous,
                next,
                index + 2 == block.stmts.len(),
                block.stmts.len(),
                self.config,
            );
            if let Some(rule) = rule {
                self.boundary(previous.span(), next.span(), rule);
            }
        }
    }
}

impl<'ast> Visit<'ast> for Analyzer<'_> {
    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.block(block);
        visit::visit_block(self, block);
    }

    fn visit_item(&mut self, item: &'ast syn::Item) {
        let attrs = match item {
            syn::Item::Fn(i) => &i.attrs,
            syn::Item::Mod(i) => &i.attrs,
            syn::Item::Impl(i) => &i.attrs,
            syn::Item::Trait(i) => &i.attrs,
            _ => return visit::visit_item(self, item),
        };
        if !skip(attrs) {
            visit::visit_item(self, item);
        }
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        if !skip(&item.attrs) {
            visit::visit_impl_item_fn(self, item);
        }
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        if !skip(&item.attrs) {
            visit::visit_trait_item_fn(self, item);
        }
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        let attrs = expr_attrs(expr);
        if !skip(attrs) {
            visit::visit_expr(self, expr);
        }
    }

    fn visit_expr_match(&mut self, expr: &'ast syn::ExprMatch) {
        if self.config.enabled("match-arm-spacing") {
            for pair in expr.arms.windows(2) {
                if matches!(&*pair[0].body, Expr::Block(block) if block.block.stmts.len() > self.config.short_block_max_statements)
                {
                    self.boundary(pair[0].span(), pair[1].span(), "match-arm-spacing");
                }
            }
        }
        visit::visit_expr_match(self, expr);
    }
}
