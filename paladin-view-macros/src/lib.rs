use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(DynView)]
pub fn derive(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, .. } = parse_macro_input!(input);
    let output = quote! {
        impl ::paladin_view::DynView for #ident {
            fn register(&self, registry: &mut ::paladin_view::reflect::TypeRegistry) {
                registry.register::<#ident>();
                #ident::register_type_dependencies(registry);
            }

            fn dyn_cmp(&self, child_id: ::paladin_view::taffy::NodeId, tree: &mut ::paladin_view::app::WidgetTree, registry: &mut ::paladin_view::reflect::TypeRegistry) {
                ::paladin_view::app::iter_elements_cmp(tree, child_id, self.build(), registry)
            }
        }
    };
    output.into()
}

#[proc_macro_attribute]
pub fn view(_metadata: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    let input: proc_macro2::TokenStream = input.into();

    let output = quote! {
        use ::paladin_view::reflect::*;
        #[derive(Reflect, DynView)]
        #input
    };
    output.into()
}
