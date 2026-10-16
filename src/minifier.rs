use anyhow::Result;

pub trait Minifier {
    fn minify(&self, input: &str) -> Result<String>;
}
