//! Bundle and optimize css

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use lightningcss::visitor::Visit;

use super::{CSS_OUTPUT_NAME, options, utils};
use crate::prelude::*;
use crate::project_gen::FEATURE_BUNDLE;

/// Collect css from the stdout of a custom bundle build
pub fn collect_css(
    config: &options::BuildConfig,
    parse_result: &super::wasm_parser::WasmParseResult,
) -> Result<PathBuf> {
    let mut css_content = extract_css()?;

    if config.profile == options::BuildProfile::Release {
        css_content = optimize_css(&css_content, parse_result)?;
    }

    let output_path = config.dist.join(CSS_OUTPUT_NAME);
    fs::write(&output_path, css_content)?;

    Ok(output_path)
}

/// Extract the css from the binary
fn extract_css() -> Result<String> {
    let spinner = utils::create_spinner("🎨 Extracting css")?;

    let mut command = Command::new("cargo");
    command
        .arg("run")
        .args(["--features", FEATURE_BUNDLE])
        .args(["--color", "always"]);

    utils::run_with_spinner(command, spinner)
}

/// Optimize the given css string
fn optimize_css(
    raw_css_content: &str,
    parse_result: &super::wasm_parser::WasmParseResult,
) -> Result<String> {
    let mut styles = lightningcss::stylesheet::StyleSheet::parse(
        raw_css_content,
        lightningcss::stylesheet::ParserOptions {
            filename: String::from("<BUNDLED CSS>.css"),
            css_modules: None,
            source_index: 0,
            error_recovery: false,
            warnings: None,
            flags: lightningcss::stylesheet::ParserFlags::empty(),
        },
    )
    .map_err(|err| anyhow!("Failed to parse css {err}"))?;

    let wasm_strings = &parse_result.data_strings;
    let mut unused_symbols = get_symbols(&mut styles);
    // `wasm_strings` is a vec of data sections, so we need to check if the symbol is in any of
    // them as wasm optimizes multiple string literals to the same section
    unused_symbols.retain(|symbol| wasm_strings.iter().all(|x| !x.contains(symbol)));

    let targets = lightningcss::targets::Targets::default();
    styles.minify(lightningcss::stylesheet::MinifyOptions {
        targets,
        unused_symbols,
    })?;

    let css_content = styles.to_css(lightningcss::printer::PrinterOptions {
        analyze_dependencies: None,
        minify: true,
        project_root: None,
        pseudo_classes: None,
        targets,
    })?;
    let css_content = css_content.code;

    Ok(css_content)
}

/// Visitor to extract symbosl from a stylesheet
struct SymbolVisitor {
    /// The collected symbols
    symbols: HashSet<String>,
    /// Symbols the should always be kept
    keep: HashSet<String>,
}

impl<'css> lightningcss::visitor::Visitor<'css> for SymbolVisitor {
    type Error = std::convert::Infallible;
    fn visit_types(&self) -> lightningcss::visitor::VisitTypes {
        lightningcss::visit_types!(SELECTORS | RULES)
    }

    fn visit_rule(
        &mut self,
        rule: &mut lightningcss::rules::CssRule<'css>,
    ) -> std::result::Result<(), Self::Error> {
        if let lightningcss::rules::CssRule::Unknown(unknown_rule) = rule
            && unknown_rule.name == "keep"
        {
            let tokens = &unknown_rule.prelude.0;
            if let Some(token) = tokens.first() {
                match token {
                    lightningcss::properties::custom::TokenOrValue::Token(
                        lightningcss::properties::custom::Token::Ident(ident),
                    ) => {
                        let ident = ident.to_string();
                        self.keep.insert(ident);
                    }
                    lightningcss::properties::custom::TokenOrValue::DashedIdent(ident) => {
                        let ident = ident.to_string();
                        self.keep.insert(ident);
                    }
                    _ => (),
                }
            }
            *rule = lightningcss::rules::CssRule::Ignored;
        }
        rule.visit_children(self)
    }

    fn visit_selector(
        &mut self,
        selector: &mut lightningcss::selector::Selector<'css>,
    ) -> std::result::Result<(), Self::Error> {
        use lightningcss::selector::Component;
        for part in selector.iter_mut_raw_match_order() {
            match part {
                Component::Class(class) => {
                    self.symbols.insert(class.to_string());
                }
                Component::ID(id) => {
                    self.symbols.insert(id.to_string());
                }
                Component::Negation(lst) | Component::Is(lst) | Component::Where(lst) => {
                    for sub_selector in lst.iter_mut() {
                        self.visit_selector(sub_selector)?;
                    }
                }
                _ => (),
            }
        }
        Ok(())
    }

    fn visit_selector_list(
        &mut self,
        selectors: &mut lightningcss::selector::SelectorList<'css>,
    ) -> std::result::Result<(), Self::Error> {
        for selector in &mut selectors.0 {
            self.visit_selector(selector)?;
        }
        Ok(())
    }
}

/// Get the symbols to DCE in a style sheet
fn get_symbols(stylesheet: &mut lightningcss::stylesheet::StyleSheet) -> HashSet<String> {
    let mut visitor = SymbolVisitor {
        symbols: HashSet::new(),
        keep: HashSet::new(),
    };
    let _ = stylesheet.visit(&mut visitor);
    visitor.symbols.difference(&visitor.keep).cloned().collect()
}
