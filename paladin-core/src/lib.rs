mod editor;

pub use editor::*;

pub mod lsp;
pub mod ts;

pub type Result<T> = miette::Result<T>;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
