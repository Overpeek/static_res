use core::panic;
use globwalk::DirEntry;
use proc_macro::TokenStream as TokenStream1;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};
use std::{collections::HashMap, ffi::OsString};
use syn::{parse_macro_input, AttributeArgs, ItemMod, Lit, NestedMeta, VisPublic, Visibility};

fn convert_string_to_ident(string: &str) -> Ident {
    let string = string.replace(".", "_");
    let string = string.replace(",", "_");
    let string = string.replace("-", "_");
    Ident::new(&string, Span::call_site())
}

#[derive(Default)]
struct Directory {
    name: OsString,
    dirs: HashMap<OsString, Directory>,
    files: HashMap<OsString, DirEntry>,
    vis: Option<Visibility>,
}

impl ToTokens for Directory {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut dirs = quote! {};
        let mut files = quote! {};

        for (_, dir) in self.dirs.iter() {
            dirs = quote! {
                #dirs
                #dir
            };
        }

        for (name, file) in self.files.iter() {
            let name = convert_string_to_ident(&name.to_string_lossy());
            let path = file.path().canonicalize().unwrap();
            let path = path.to_string_lossy();
            files = quote! {
                #files
                pub const #name: &[u8] = include_bytes!(#path);
            };
        }

        let name = convert_string_to_ident(&self.name.to_string_lossy());
        let vis = self.vis.clone().unwrap_or_else(|| {
            Visibility::Public(VisPublic {
                pub_token: Default::default(),
            })
        });
        tokens.extend(quote! {
            #vis mod #name {
                #dirs
                #files
            }
        });
    }
}

/// # `static_res!` macro
/// ```
/// # use static_res::static_res;
/// #[static_res("tests/**")]
/// pub mod res {}
///
/// # fn main() {
/// assert!(res::tests::test_txt == include_bytes!("../tests/test.txt"));
/// assert!(res::tests::folder::test_txt == b"yet another test");
/// # }
/// ```
#[proc_macro_attribute]
pub fn static_res(attr: TokenStream1, tokens: TokenStream1) -> TokenStream1 {
    // mod
    let item_mod = parse_macro_input!(tokens as ItemMod);
    let vis = item_mod.vis;
    let ident = item_mod.ident;

    // attr
    let attr = parse_macro_input!(attr as AttributeArgs);
    assert!(attr.len() == 1, "Exactly one attribute was expected");
    let pattern = match attr.first().unwrap() {
        NestedMeta::Lit(Lit::Str(pattern)) => pattern.value(),
        _ => panic!("String literal was expected"),
    };

    // build a directory tree

    let mut files = Directory {
        name: ident.to_string().into(),
        vis: Some(vis),
        ..Default::default()
    };
    for file in globwalk::glob_builder(pattern)
        .max_depth(10)
        .follow_links(true)
        .build()
        .unwrap()
        .flatten()
    {
        let path = file.path();
        let mut files = &mut files;

        let segments = path.iter().collect::<Vec<_>>();
        let (last, dirs) = segments.split_last().unwrap();

        for dir in dirs {
            if dir.to_string_lossy() == "." {
                continue;
            }
            let dir = dir.to_os_string();
            files = files.dirs.entry(dir.clone()).or_insert_with(|| Directory {
                name: dir,
                ..Default::default()
            });
        }

        if path.is_dir() {
            files.dirs.insert(
                last.to_os_string(),
                Directory {
                    name: last.to_os_string(),
                    ..Default::default()
                },
            );
        } else {
            files.files.insert(last.to_os_string(), file);
        }
    }

    // output the tree as tokens

    files.to_token_stream().into()
}
