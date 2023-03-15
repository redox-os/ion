extern crate proc_macro;
use darling::{util::Flag, FromMeta};
use proc_macro::TokenStream;
use quote::quote;
use std::{fs::File, io::Write};
use syn::Ident;

#[derive(Debug, FromMeta)]
struct MacroArgs {
    #[darling(default)]
    names:             Option<String>,
    #[darling(rename = "man")]
    help:              String,
    #[darling(default)]
    authors:           Flag,
    #[darling(rename = "desc")]
    short_description: String,
}

// TODO: It would be better if Man pages could be parsed of comments

/// Implements automatic help check in annotated function.
/// Builds man page of the builtin for the ion manual if compiled with feature `man`.
#[proc_macro_attribute]
pub fn builtin(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let attrs = syn::parse_macro_input!(attr as syn::AttributeArgs);
    let syn::ItemFn { vis, sig, block, .. } = &input;
    let syn::Signature { ident, fn_token, inputs, output, .. } = sig;
    //
    let args = match MacroArgs::from_list(&attrs) {
        Ok(v) => v,
        Err(e) => return e.write_errors().into(),
    };

    let (man, help, name) = build_man_page(&ident, args);

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

/// Some interactive builtins require access to additional values then just args and shell.
/// This proc macro attribute is used on functions which return a closure as builtin callback.
/// You have to insert the check for help, aka -h or --help, in the closure yourself.
#[proc_macro_attribute]
pub fn builtin_interactive(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let attrs = syn::parse_macro_input!(attr as syn::AttributeArgs);
    let syn::ItemFn { vis, sig, block, .. } = &input;
    let syn::Signature { ident, fn_token, inputs, output, .. } = sig;

    let args = match MacroArgs::from_list(&attrs) {
        Ok(v) => v,
        Err(e) => return e.write_errors().into(),
    };

    let (man, help, name) = build_man_page(&ident, args);

    let result = quote! {
        #[doc = #help]
        #vis #fn_token #name(#inputs) #output {
            const HELP_PAGE: &str = #man;
            #block
        }
    };

    result.into()
}

fn build_man_page(ident: &Ident, args: MacroArgs) -> (String, String, Ident) {
    let name = quote::format_ident!("builtin_{}", &ident, span = ident.span(),);

    let help = args.help.trim();
    let names = args.names.unwrap_or_else(|| ident.to_string());

    let bugs = "BUGS
    Please report all bugs at https://gitlab.redox-os.org/redox-os/ion/issues.
    Ion is still in active development and help in finding bugs is much appreciated!";

    let extra = "

AUTHORS
    The Ion developers, under the Redox OS organisation"
        .to_string();

    let man = format!(
        "NAME\n    {names} - {short_description}\n\n{help}\n\n{bugs}{extra}",
        names = names,
        short_description = args.short_description,
        help = help,
        bugs = bugs,
        extra = if args.authors.is_none() { &extra } else { "" },
    );
    let help = format!("{} - {}\n\n```txt\n{}\n```", names, args.short_description, help);

    if cfg!(feature = "man") {
        let mut man = File::create(format!("manual/builtins/{}.1", &ident)).unwrap();
        man.write_all(help.as_bytes()).unwrap();
    }

    (man, help, name)
}
