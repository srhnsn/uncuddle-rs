use syn::Expr;

pub(super) fn skip(attrs: &[syn::Attribute]) -> bool {
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

pub(super) fn expr_attrs(expr: &Expr) -> &[syn::Attribute] {
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
