use crate::minifier::Minifier;
use anyhow::{Result, anyhow};
use swc_common::{FileName, GLOBALS, SourceMap, sync};
use swc_ecma_codegen::{Config, Emitter, text_writer::JsWriter};
use swc_ecma_minifier::{
    optimize,
    option::{ExtraOptions, MinifyOptions},
};
use swc_ecma_parser::{EsSyntax, Syntax, parse_file_as_program};

#[derive(Clone)]
pub struct SwcJsMinifier;

impl Minifier for SwcJsMinifier {
    fn minify(&self, input: &str) -> Result<String> {
        GLOBALS.set(&Default::default(), || {
            let cm: sync::Lrc<SourceMap> = Default::default();
            let ast = parse_file_as_program(
                &cm.new_source_file(FileName::Anon.into(), input.to_string()),
                Syntax::Es(EsSyntax::default()),
                Default::default(),
                None,
                &mut vec![],
            )
            .map_err(|e| anyhow!("{}", e.kind().msg()))?;
            let unresolved_mark = swc_common::Mark::new();
            let top_level_mark = swc_common::Mark::new();
            let output = optimize(
                ast,
                cm.clone(),
                None,
                None,
                &MinifyOptions {
                    compress: Some(Default::default()),
                    ..Default::default()
                },
                &ExtraOptions {
                    unresolved_mark,
                    top_level_mark,
                    mangle_name_cache: None,
                },
            );
            let mut buf = vec![];
            (Emitter {
                cfg: Config::default().with_minify(true),
                comments: None,
                cm: cm.clone(),
                wr: JsWriter::new(cm.clone(), "\n", &mut buf, None),
            })
            .emit_program(&output)?;
            Ok(String::from_utf8(buf)?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minifies_js() {
        let src = "function hello() {\n  console.log(1 + 2);\n}\nhello();\n";
        let out = SwcJsMinifier.minify(src).unwrap();
        assert!(out.len() < src.len(), "{out}");
        assert_eq!(out, "console.log(3);");
    }

    #[test]
    fn rejects_invalid_js() {
        let err = SwcJsMinifier.minify("function (").unwrap_err();
        assert!(!err.to_string().is_empty());
    }
}
