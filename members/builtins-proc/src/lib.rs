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
    let mut authors = true;

    let name = syn::Ident::new(&format!("builtin_{}", &ident), input.ident.span());

    for attr in attrs {
        match attr {
            syn::NestedMeta::Meta(syn::Meta::NameValue(ref attr)) if attr.ident == "man" => {
                if let syn::Lit::Str(h) = &attr.lit {
                    help = Some(h.value());
                } else {
                    panic!("`man` attribute should be a string variable");
                }
            }
            syn::NestedMeta::Meta(syn::Meta::NameValue(ref attr)) if attr.ident == "desc" => {
                if let syn::Lit::Str(h) = &attr.lit {
                    short_description = Some(h.value());
                } else {
                    panic!("`desc` attribute should be a string variable");
                }
            }
            syn::NestedMeta::Meta(syn::Meta::NameValue(ref attr)) if attr.ident == "names" => {
                if let syn::Lit::Str(h) = &attr.lit {
                    names = Some(h.value());
                } else {
                    panic!("`desc` attribute should be a string variable");
                }
            }
            syn::NestedMeta::Meta(syn::Meta::Word(ref ident)) if ident == "no_authors" => {
                authors = false;
            }
            _ => panic!("Only the `man` and `desc` attributes are allowed"),
        }
    }
    let help = help.expect("A man page is required! Please add an attribute with name `man`");
    let help = help.trim();
    let short_description = short_description
        .expect("A short description is required! Please add an attribute with name `desc`");
    let names = names.unwrap_or_else(|| ident.to_string());

    let bugs = "BUGS
    Please report all bugs at https://gitlab.redox-os.org/redox-os/ion/issues.
    Ion is still in active development and help in finding bugs is much appreciated!";

    let extra = format!(
        "

AUTHORS
    The Ion developers, under the Redox OS organisation"
    );
    let man = format!(
        "NAME\n    {names} - {short_description}\n\n{help}\n\n{bugs}{extra}",
        names = names,
        short_description = short_description,
        help = help,
        bugs = bugs,
        extra = if authors { &extra } else { "" },
    );
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
