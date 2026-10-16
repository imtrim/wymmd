use crate::minifier::Minifier;
use anyhow::{Result, anyhow};
use std::sync::Arc;
use swc_common::{FileName, SourceMap};
use swc_css_codegen::{
    CodeGenerator, CodegenConfig, Emit,
    writer::basic::{BasicCssWriter, BasicCssWriterConfig},
};
use swc_css_minifier::{minify, options::MinifyOptions};
use swc_css_parser::{parse_file, parser::ParserConfig};

#[derive(Clone)]
pub struct SwcCssMinifier;

impl Minifier for SwcCssMinifier {
    fn minify(&self, input: &str) -> Result<String> {
        let mut ast = parse_file(
            &Arc::new(SourceMap::default())
                .new_source_file(FileName::Anon.into(), input.to_string()),
            None,
            ParserConfig::default(),
            &mut vec![],
        )
        .map_err(|e| anyhow!("{}", e.message()))?;
        minify(&mut ast, MinifyOptions::default());
        let mut output = String::new();
        CodeGenerator::new(
            BasicCssWriter::new(&mut output, None, BasicCssWriterConfig::default()),
            CodegenConfig { minify: true },
        )
        .emit(&ast)?;
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minifies_css() {
        let out = SwcCssMinifier
            .minify("body { /* note */ color: red; }")
            .unwrap();
        assert_eq!(out, "body{color:red}");
    }

    #[test]
    fn recovers_unclosed_rule() {
        let out = SwcCssMinifier.minify("body {").unwrap();
        assert_eq!(out, "");
    }
}
