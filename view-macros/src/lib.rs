use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Register)]
pub fn derive(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, .. } = parse_macro_input!(input);
    let output = quote! {
        impl ::view::Register for #ident {
            fn register(&self, registry: &mut ::bevy_reflect::TypeRegistry) {
                registry.register::<#ident>();
                #ident::register_type_dependencies(registry);
            }
        }
    };
    output.into()
}

#[proc_macro_attribute]
pub fn view(_metadata: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    let input: proc_macro2::TokenStream = input.into();

    let output = quote! {
        #[derive(Reflect, Register)]
        #input
    };
    output.into()
}
