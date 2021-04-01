mod calls;
mod cli_commands;
mod responses;
mod utils;

use proc_macro::TokenStream;

#[proc_macro]
pub fn declare_all_rpc_methods(input: TokenStream) -> TokenStream {
    let input_ast = syn::parse_macro_input!(input as syn::ItemExternCrate);
    dbg!(&input_ast);
    quote::quote!("a").into()
}

#[proc_macro]
pub fn declare_rpc_client_methods(input: TokenStream) -> TokenStream {
    utils::make_code(input.into(), calls::make_call).into()
}

#[proc_macro]
pub fn declare_rpc_response_types(_input: TokenStream) -> TokenStream {
    responses::declare_rpc_response_types().into()
}

#[proc_macro]
pub fn declare_rcli_command_types(input: TokenStream) -> TokenStream {
    utils::make_code(input.into(), cli_commands::make_command).into()
}
