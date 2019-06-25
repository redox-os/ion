extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn;

// TODO: It would be better if Man pages could be parsed of comments
#[proc_macro_attribute]
pub fn builtin(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let attrs = syn::parse_macro_input!(attr as syn::AttributeArgs);
    let syn::ItemFn { vis, decl, block, ident, .. } = &input;
    let syn::FnDecl { ref fn_token, ref inputs, ref output, .. } = **decl;
    let mut help = None;
    let mut short_description = None;
    let mut names = None;

    let name = syn::Ident::new(&format!("builtin_{}", &ident), input.ident.span());

    for attr in attrs {
        if let syn::NestedMeta::Meta(syn::Meta::NameValue(attr)) = attr {
            if attr.ident == "man" {
                if let syn::Lit::Str(h) = &attr.lit {
                    help = Some(h.value());
                } else {
                    panic!("`man` attribute should be a string variable");
                }
            } else if attr.ident == "desc" {
                if let syn::Lit::Str(h) = &attr.lit {
                    short_description = Some(h.value());
                } else {
                    panic!("`desc` attribute should be a string variable");
                }
            } else if attr.ident == "names" {
                if let syn::Lit::Str(h) = &attr.lit {
                    names = Some(h.value());
                } else {
                    panic!("`desc` attribute should be a string variable");
                }
            } else {
                panic!("Only the `man` and `desc` attributes are allowed");
            }
        } else {
            panic!("Only the `man` and `desc` attributes are allowed");
        }
    }
    let help = help.expect("A man page is required! Please add an attribute with name `man`");
    let help = help.trim();
    let short_description = short_description
        .expect("A short description is required! Please add an attribute with name `desc`");
    let names = names.unwrap_or_else(|| ident.to_string());
    let man = format!("NAME\n    {} - {}\n\n{}", names, short_description, help);
    let help = format!("{} - {}\n\n```txt\n{}\n```", names, short_description, help);

    let result = quote! {
        #[doc = #help]
        #vis #fn_token #name(#inputs) #output {
            if ion_shell::builtins::man_pages::check_help(args, #man) {
                return ion_shell::builtins::Status::SUCCESS;
            }
            #block
        }
    };
    result.into()
}
