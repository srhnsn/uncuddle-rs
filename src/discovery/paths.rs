//! Parse static module path alternatives, including nested cfg_attr, without
//! evaluating features or platform conditions.
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

pub(super) struct Paths {
    pub explicit: Vec<PathBuf>,
    pub use_default: bool,
}

pub(super) fn resolve(attrs: &[syn::Attribute], base: &Path) -> Result<Paths> {
    let mut paths = Paths {
        explicit: Vec::new(),
        use_default: true,
    };

    for attr in attrs {
        collect(&attr.meta, base, false, &mut paths)?;
    }

    paths.explicit.sort();
    paths.explicit.dedup();

    Ok(paths)
}

fn collect(meta: &syn::Meta, base: &Path, conditional: bool, paths: &mut Paths) -> Result<()> {
    if meta.path().is_ident("path") {
        let syn::Meta::NameValue(value) = meta else {
            bail!("unsupported non-literal module path attribute");
        };
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(path),
            ..
        }) = &value.value
        else {
            bail!("unsupported non-literal module path attribute");
        };
        paths.explicit.push(base.join(path.value()));

        if !conditional {
            paths.use_default = false;
        }
    } else if meta.path().is_ident("cfg_attr") {
        let syn::Meta::List(list) = meta else {
            bail!("invalid cfg_attr attribute");
        };
        let nested = list.parse_args_with(
            syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
        )?;

        for meta in nested.iter().skip(1) {
            collect(meta, base, true, paths)?;
        }
    }

    Ok(())
}
