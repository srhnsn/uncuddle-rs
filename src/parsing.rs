//! Keep source byte positions intact, including files with a BOM or shebang.
//! Syn has no edition argument; legacy 2015 keyword identifiers are normalized
//! in the parser's token stream only, never in the user's source.
use proc_macro2::{Group, Ident, TokenStream, TokenTree};
use std::borrow::Cow;

pub fn parse(source: &str, edition: &str) -> Result<syn::File, syn::Error> {
    let input = prepare(source);

    if edition != "2015" {
        return syn::parse_file(&input);
    }

    let mut tokens: TokenStream = input.parse()?;

    loop {
        match syn::parse2(tokens.clone()) {
            Ok(file) => return Ok(file),
            Err(error) => {
                let mut changed = false;

                tokens = normalize(tokens, &error.span().byte_range(), &mut changed);

                if !changed {
                    return Err(error);
                }
            }
        }
    }
}

fn prepare(source: &str) -> Cow<'_, str> {
    let bom = if source.starts_with('\u{feff}') { 3 } else { 0 };
    let rest = &source[bom..];
    let shebang = rest.starts_with("#!") && !starts_inner_attribute(&rest[2..]);

    if bom == 0 && !shebang {
        return Cow::Borrowed(source);
    }

    let end = if shebang {
        rest.find('\n').map_or(source.len(), |n| bom + n)
    } else {
        bom
    };

    let mut input = source.as_bytes().to_vec();
    input[..end].fill(b' ');

    Cow::Owned(
        String::from_utf8(input).expect("replacing a complete UTF-8 prefix with ASCII is valid"),
    )
}

fn starts_inner_attribute(mut rest: &str) -> bool {
    loop {
        rest = rest.trim_start();

        if rest.starts_with("//") {
            rest = rest.find('\n').map_or("", |n| &rest[n..]);
        } else if rest.starts_with("/*") {
            let bytes = rest.as_bytes();
            let mut index = 2;
            let mut depth = 1;

            while index < bytes.len() && depth > 0 {
                if bytes[index..].starts_with(b"/*") {
                    depth += 1;
                    index += 2;
                } else if bytes[index..].starts_with(b"*/") {
                    depth -= 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }

            if depth != 0 {
                return false;
            }

            rest = &rest[index..];
        } else {
            return rest.starts_with('[');
        }
    }
}

fn normalize(
    tokens: TokenStream,
    error: &std::ops::Range<usize>,
    changed: &mut bool,
) -> TokenStream {
    tokens
        .into_iter()
        .map(|token| match token {
            TokenTree::Ident(ident)
                if ident.span().byte_range() == *error
                    && matches!(ident.to_string().as_str(), "async" | "await" | "dyn") =>
            {
                *changed = true;

                TokenTree::Ident(Ident::new_raw(&ident.to_string(), ident.span()))
            }
            TokenTree::Group(group) => {
                let mut replacement =
                    Group::new(group.delimiter(), normalize(group.stream(), error, changed));
                replacement.set_span(group.span());

                TokenTree::Group(replacement)
            }

            token => token,
        })
        .collect()
}
