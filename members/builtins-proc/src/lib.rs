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

    let name = syn::Ident::new(&format!("builtin_{}", &ident), input.ident.span());

    let help = attrs
        .iter()
        .filter_map(|meta| {
            if let syn::NestedMeta::Meta(syn::Meta::NameValue(attr)) = meta {
                if attr.ident == "man" {
                    if let syn::Lit::Str(help) = &attr.lit {
                        Some(help.value())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        })
        .next()
        .expect("A man page is required! Please add a documentation comment");
    let man = format!("NAME\n    {} - {}", ident, help.trim());

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
