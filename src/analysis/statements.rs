use std::collections::BTreeSet;
use syn::ext::IdentExt;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, Stmt};

#[derive(PartialEq, Eq, Clone, Copy)]
pub(super) enum Kind {
    Binding,
    Assignment,
    Expr,
    Macro,
    Control,
    Exit,
    Item,
}

pub(super) fn kind(stmt: &Stmt) -> Kind {
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

pub(super) fn group_transition(left: Kind, right: Kind) -> bool {
    let binding = |k| matches!(k, Kind::Binding | Kind::Assignment);
    let expression = |k| matches!(k, Kind::Expr | Kind::Macro);

    (binding(left) && expression(right)) || (expression(left) && binding(right))
}

pub(super) fn has_block_value(stmt: &Stmt) -> bool {
    let expr = match stmt {
        Stmt::Local(local) => local.init.as_ref().map(|init| &*init.expr),
        Stmt::Expr(expr, _) => Some(expr),
        _ => None,
    };
    expr.is_some_and(|expr| {
        if expr.span().start().line == expr.span().end().line {
            return false;
        }

        matches!(
            expr,
            Expr::Block(_)
                | Expr::Async(_)
                | Expr::Unsafe(_)
                | Expr::Closure(_)
                | Expr::If(_)
                | Expr::Match(_)
                | Expr::Loop(_)
        )
    })
}

pub(super) fn related(previous: &Stmt, next: &Stmt) -> bool {
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
        self.names.insert(pat.ident.unraw().to_string());
        visit::visit_pat_ident(self, pat);
    }
    fn visit_expr_path(&mut self, expr: &'ast syn::ExprPath) {
        if expr.qself.is_none()
            && expr.path.leading_colon.is_none()
            && expr.path.segments.len() == 1
        {
            self.names
                .insert(expr.path.segments[0].ident.unraw().to_string());
        }
    }
    // Bodies with their own bindings are not evidence of immediate consumption.
    fn visit_expr_closure(&mut self, _: &'ast syn::ExprClosure) {}
    fn visit_expr_block(&mut self, _: &'ast syn::ExprBlock) {}
    fn visit_expr_async(&mut self, _: &'ast syn::ExprAsync) {}
    fn visit_block(&mut self, _: &'ast syn::Block) {}
    fn visit_macro(&mut self, _: &'ast syn::Macro) {}
}
