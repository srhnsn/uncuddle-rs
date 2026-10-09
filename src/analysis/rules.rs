//! Pure boundary policies. The visitor chooses locations; these predicates
//! decide which rule applies. Registry order resolves overlapping rules.
use super::statements::{Kind, group_transition, has_block_value, kind, related};
use crate::config::Config;
use syn::Stmt;

struct Boundary<'a> {
    previous: &'a Stmt,
    next: &'a Stmt,
    left: Kind,
    right: Kind,
    is_tail: bool,
    is_large: bool,
}

type Predicate = fn(&Boundary<'_>) -> bool;

const POLICIES: &[(&str, Predicate)] = &[
    ("before-control-flow", |b| b.right == Kind::Control),
    ("after-control-flow", |b| b.left == Kind::Control),
    ("before-exit", |b| b.is_large && b.right == Kind::Exit),
    ("before-tail-expression", |b| {
        b.is_large && b.is_tail && b.right != Kind::Exit
    }),
    ("assignment-kinds", |b| {
        matches!(
            (b.left, b.right),
            (Kind::Binding, Kind::Assignment) | (Kind::Assignment, Kind::Binding)
        )
    }),
    ("local-item-spacing", |b| {
        (b.left == Kind::Item) != (b.right == Kind::Item)
    }),
    ("after-block-value", |b| {
        has_block_value(b.previous) && !related(b.previous, b.next)
    }),
    ("statement-groups", |b| {
        group_transition(b.left, b.right) && !related(b.previous, b.next)
    }),
];

pub(super) fn for_boundary(
    previous: &Stmt,
    next: &Stmt,
    is_last: bool,
    block_size: usize,
    config: &Config,
) -> Option<&'static str> {
    let boundary = Boundary {
        previous,
        next,
        left: kind(previous),
        right: kind(next),
        is_tail: is_last && matches!(next, Stmt::Expr(_, None)),
        is_large: block_size > config.short_block_max_statements,
    };
    POLICIES
        .iter()
        .find_map(|(id, predicate)| (config.enabled(id) && predicate(&boundary)).then_some(*id))
}
