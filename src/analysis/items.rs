use syn::spanned::Spanned;

/// Function bodies need separation even when their contents are short. Trait
/// signatures without bodies are declarations and can remain grouped together.
pub(super) trait FunctionItem: Spanned {
    fn is_function_definition(&self) -> bool;
}

impl FunctionItem for syn::Item {
    fn is_function_definition(&self) -> bool {
        matches!(self, syn::Item::Fn(_))
    }
}

impl FunctionItem for syn::ImplItem {
    fn is_function_definition(&self) -> bool {
        matches!(self, syn::ImplItem::Fn(_))
    }
}

impl FunctionItem for syn::TraitItem {
    fn is_function_definition(&self) -> bool {
        matches!(self, syn::TraitItem::Fn(function) if function.default.is_some())
    }
}
