use core::panic;
use globwalk::DirEntry;
use proc_macro::TokenStream as TokenStream1;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};
use std::{collections::HashMap, ffi::OsString};
use syn::{parse_macro_input, AttributeArgs, Lit, NestedMeta};

fn convert_string_to_ident(string: &str) -> Ident {
    let string = string.replace('.', "_");
    let string = string.replace(',', "_");
    let string = string.replace('-', "_");
    Ident::new(&string, Span::call_site())
}

#[derive(Default)]
struct Directory {
    name: OsString,
    dirs: HashMap<OsString, Directory>,
    files: HashMap<OsString, DirEntry>,
}

impl ToTokens for Directory {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut dirs = quote! {};
        let mut files = quote! {};

        for (_, dir) in self.dirs.iter() {
            let name = convert_string_to_ident(dir.name.to_string_lossy().as_ref());
            dirs = quote! {
                #dirs
                pub mod #name {
                    #dir
                }
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

        tokens.extend(quote! {
            #dirs
            #files
        });
    }
}

/// # `static_res!` macro
/// ```
/// # use static_res::static_res;
/// static_res! { "tests/**" }
///
/// # fn main() {
/// assert!(tests::test_txt == include_bytes!("../tests/test.txt"));
/// assert!(tests::folder::test_txt == b"yet another test");
/// # }
/// ```
#[proc_macro]
pub fn static_res(attr: TokenStream1) -> TokenStream1 {
    // attr
    let attr = parse_macro_input!(attr as AttributeArgs);
    assert!(attr.len() == 1, "Exactly one attribute was expected");
    let pattern = match attr.first().unwrap() {
        NestedMeta::Lit(Lit::Str(pattern)) => pattern.value(),
        _ => panic!("String literal was expected"),
    };

    // build a directory tree

    let mut files = Directory::default();
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
