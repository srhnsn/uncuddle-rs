use crate::config::Config;
use crate::diagnostic::Diagnostic;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, Stmt};

/// Analyze source without compiling or invoking rustfmt. Macro contents are
/// opaque; edits are only proposed at boundaries between AST siblings.
pub fn analyze(path: &Path, source: &str, config: &Config) -> Result<Vec<Diagnostic>, syn::Error> {
    let ast = crate::parse(source)?;
    let mut analyzer = Analyzer {
        path,
        source,
        config,
        diagnostics: BTreeMap::new(),
    };
    analyzer.visit_file(&ast);
    Ok(analyzer.diagnostics.into_values().collect())
}

struct Analyzer<'a> {
    path: &'a Path,
    source: &'a str,
    config: &'a Config,
    diagnostics: BTreeMap<usize, Diagnostic>,
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
        let before = &self.source[..start];
        let line = before.bytes().filter(|b| *b == b'\n').count() + 1;
        let column = before.rsplit('\n').next().unwrap().chars().count() + 1;
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
            let left = kind(previous);
            let right = kind(next);
            let rule = if right == Kind::Control && self.config.enabled("before-control-flow") {
                Some("before-control-flow")
            } else if left == Kind::Control && self.config.enabled("after-control-flow") {
                Some("after-control-flow")
            } else if block.stmts.len() > self.config.short_block_max_statements
                && right == Kind::Exit
                && self.config.enabled("before-exit")
            {
                Some("before-exit")
            } else if block.stmts.len() > self.config.short_block_max_statements
                && index + 2 == block.stmts.len()
                && matches!(next, Stmt::Expr(_, None))
                && right != Kind::Exit
                && self.config.enabled("before-tail-expression")
            {
                Some("before-tail-expression")
            } else if self.config.enabled("assignment-kinds")
                && matches!(
                    (left, right),
                    (Kind::Binding, Kind::Assignment) | (Kind::Assignment, Kind::Binding)
                )
            {
                Some("assignment-kinds")
            } else if self.config.enabled("local-item-spacing")
                && (left == Kind::Item) != (right == Kind::Item)
            {
                Some("local-item-spacing")
            } else if self.config.enabled("after-block-value")
                && has_block_value(previous)
                && !related(previous, next)
            {
                Some("after-block-value")
            } else if self.config.enabled("statement-groups")
                && group_transition(left, right)
                && !related(previous, next)
            {
                Some("statement-groups")
            } else {
                None
            };
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

fn skip(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        let segments: Vec<_> = attr
            .path()
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect();
        segments == ["rustfmt", "skip"]
    })
}

fn expr_attrs(expr: &Expr) -> &[syn::Attribute] {
    match expr {
        Expr::Async(e) => &e.attrs,
        Expr::Block(e) => &e.attrs,
        Expr::Closure(e) => &e.attrs,
        Expr::ForLoop(e) => &e.attrs,
        Expr::If(e) => &e.attrs,
        Expr::Loop(e) => &e.attrs,
        Expr::Match(e) => &e.attrs,
        Expr::Unsafe(e) => &e.attrs,
        Expr::While(e) => &e.attrs,
        _ => &[],
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Kind {
    Binding,
    Assignment,
    Expr,
    Macro,
    Control,
    Exit,
    Item,
}

fn kind(stmt: &Stmt) -> Kind {
    match stmt {
        Stmt::Local(_) => Kind::Binding,
        Stmt::Item(_) => Kind::Item,
        Stmt::Macro(_) => Kind::Macro,
        Stmt::Expr(expr, _) => match expr {
            Expr::If(_) | Expr::Match(_) | Expr::ForLoop(_) | Expr::While(_) | Expr::Loop(_) => {
                Kind::Control
            }
            Expr::Return(_) | Expr::Break(_) | Expr::Continue(_) => Kind::Exit,
            Expr::Assign(_) => Kind::Assignment,
            Expr::Binary(binary)
                if matches!(
                    binary.op,
                    syn::BinOp::AddAssign(_)
                        | syn::BinOp::SubAssign(_)
                        | syn::BinOp::MulAssign(_)
                        | syn::BinOp::DivAssign(_)
                        | syn::BinOp::RemAssign(_)
                        | syn::BinOp::BitXorAssign(_)
                        | syn::BinOp::BitAndAssign(_)
                        | syn::BinOp::BitOrAssign(_)
                        | syn::BinOp::ShlAssign(_)
                        | syn::BinOp::ShrAssign(_)
                ) =>
            {
                Kind::Assignment
            }
            Expr::Macro(_) => Kind::Macro,
            _ => Kind::Expr,
        },
    }
}

fn group_transition(left: Kind, right: Kind) -> bool {
    let binding = |k| matches!(k, Kind::Binding | Kind::Assignment);
    let expression = |k| matches!(k, Kind::Expr | Kind::Macro);
    (binding(left) && expression(right)) || (expression(left) && binding(right))
}

fn has_block_value(stmt: &Stmt) -> bool {
    let expr = match stmt {
        Stmt::Local(local) => local.init.as_ref().map(|init| &*init.expr),
        Stmt::Expr(expr, _) => Some(expr),
        _ => None,
    };
    expr.is_some_and(|expr| {
        matches!(
            expr,
            Expr::Block(_) | Expr::Async(_) | Expr::Unsafe(_) | Expr::Closure(_)
        )
    })
}

fn related(previous: &Stmt, next: &Stmt) -> bool {
    let mut names = Names::default();
    match previous {
        Stmt::Local(local) => names.visit_pat(&local.pat),
        Stmt::Expr(Expr::Assign(assign), _) => names.visit_expr(&assign.left),
        Stmt::Expr(Expr::Binary(binary), _) if kind(previous) == Kind::Assignment => {
            names.visit_expr(&binary.left)
        }
        Stmt::Expr(expr, _) => names.visit_expr(expr),
        _ => {}
    }
    let mut used = Names::default();
    match next {
        Stmt::Local(local) => {
            if let Some(init) = &local.init {
                used.visit_expr(&init.expr);
            }
        }
        Stmt::Expr(expr, _) => used.visit_expr(expr),
        _ => {}
    }
    !names.names.is_disjoint(&used.names)
}

#[derive(Default)]
struct Names {
    names: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for Names {
    fn visit_pat_ident(&mut self, pat: &'ast syn::PatIdent) {
        self.names.insert(pat.ident.to_string());
        visit::visit_pat_ident(self, pat);
    }
    fn visit_expr_path(&mut self, expr: &'ast syn::ExprPath) {
        if expr.qself.is_none()
            && expr.path.leading_colon.is_none()
            && expr.path.segments.len() == 1
        {
            self.names.insert(expr.path.segments[0].ident.to_string());
        }
    }
    // Bodies with their own bindings are not evidence of immediate consumption.
    fn visit_expr_closure(&mut self, _: &'ast syn::ExprClosure) {}
    fn visit_expr_block(&mut self, _: &'ast syn::ExprBlock) {}
    fn visit_expr_async(&mut self, _: &'ast syn::ExprAsync) {}
    fn visit_macro(&mut self, _: &'ast syn::Macro) {}
}

/// Classify only the trivia between two syntax nodes. Blank lines inside
/// block comments do not count; trailing inline comments stay with the left
/// statement and standalone comments stay attached to the right statement.
struct Gap {
    has_blank: bool,
    insert_at: Option<usize>,
}

impl Gap {
    fn scan(source: &str, end: usize, start: usize) -> Option<Self> {
        let bytes = source.as_bytes();
        let mut comments = Vec::new();
        let mut index = end;
        while index < start {
            if bytes[index].is_ascii_whitespace() {
                index += 1;
            } else if bytes[index..start].starts_with(b"//") {
                let begin = index;
                while index < start && bytes[index] != b'\n' {
                    index += 1;
                }
                comments.push(begin..index);
            } else if bytes[index..start].starts_with(b"/*") {
                let begin = index;
                index += 2;
                let mut depth = 1;
                while index < start && depth > 0 {
                    if bytes[index..start].starts_with(b"/*") {
                        depth += 1;
                        index += 2;
                    } else if bytes[index..start].starts_with(b"*/") {
                        depth -= 1;
                        index += 2;
                    } else {
                        index += 1;
                    }
                }
                if depth != 0 {
                    return None;
                }
                comments.push(begin..index);
            } else {
                return None;
            }
        }
        let previous_line_start = source[..end].rfind('\n').map_or(0, |n| n + 1);
        let next_line_start = source[..start].rfind('\n').map_or(0, |n| n + 1);
        if previous_line_start == next_line_start {
            return Some(Self {
                has_blank: false,
                insert_at: None,
            });
        }
        let mut insert_at = source[end..].find('\n').map(|n| end + n + 1)?;
        // A multiline comment starting on the previous line belongs to it.
        for comment in &comments {
            if comment.start < insert_at && comment.end >= insert_at {
                insert_at = source[comment.end..]
                    .find('\n')
                    .map(|n| comment.end + n + 1)?;
            }
        }
        let mut line = source[end..].find('\n').map(|n| end + n + 1)?;
        while line < next_line_start {
            let line_end = source[line..].find('\n').map_or(source.len(), |n| line + n);
            if source[line..line_end].trim().is_empty()
                && !comments
                    .iter()
                    .any(|range| range.start <= line && line < range.end)
            {
                return Some(Self {
                    has_blank: true,
                    insert_at: None,
                });
            }
            line = line_end + 1;
        }
        Some(Self {
            has_blank: false,
            insert_at: (insert_at <= next_line_start).then_some(insert_at),
        })
    }
}
