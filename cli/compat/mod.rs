// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

mod errors;
mod esm_resolver;

use deno_core::error::AnyError;
use deno_core::located_script_name;
use deno_core::url::Url;
use deno_core::JsRuntime;

pub use esm_resolver::NodeEsmResolver;

// TODO(bartlomieju): this needs to be bumped manually for
// each release, a better mechanism is preferable, but it's a quick and dirty
// solution to avoid printing `X-Deno-Warning` headers when the compat layer is
// downloaded
static STD_URL_STR: &str = "https://deno.land/std@0.113.0/";

static SUPPORTED_MODULES: &[&str] = &[
  "assert",
  "assert/strict",
  "async_hooks",
  "buffer",
  "child_process",
  "cluster",
  "console",
  "constants",
  "crypto",
  "dgram",
  "dns",
  "domain",
  "events",
  "fs",
  "fs/promises",
  "http",
  "https",
  "module",
  "net",
  "os",
  "path",
  "path/posix",
  "path/win32",
  "perf_hooks",
  "process",
  "querystring",
  "readline",
  "stream",
  "stream/promises",
  "stream/web",
  "string_decoder",
  "sys",
  "timers",
  "timers/promises",
  "tls",
  "tty",
  "url",
  "util",
  "util/types",
  "v8",
  "vm",
  "zlib",
];

lazy_static::lazy_static! {
  static ref NODE_COMPAT_URL: String = std::env::var("DENO_NODE_COMPAT_URL").map(String::into).ok()
    .unwrap_or_else(|| STD_URL_STR.to_string());
  static ref GLOBAL_URL_STR: String = format!("{}node/global.ts", NODE_COMPAT_URL.as_str());
  pub(crate) static ref GLOBAL_URL: Url = Url::parse(&GLOBAL_URL_STR).unwrap();
  static ref MODULE_URL_STR: String = format!("{}node/module.ts", NODE_COMPAT_URL.as_str());
  pub(crate) static ref MODULE_URL: Url = Url::parse(&MODULE_URL_STR).unwrap();
  static ref COMPAT_IMPORT_URL: Url = Url::parse("flags:compat").unwrap();
}

/// Provide imports into a module graph when the compat flag is true.
pub(crate) fn get_node_imports() -> Vec<(Url, Vec<String>)> {
  vec![(COMPAT_IMPORT_URL.clone(), vec![GLOBAL_URL_STR.clone()])]
}

fn try_resolve_builtin_module(specifier: &str) -> Option<Url> {
  if SUPPORTED_MODULES.contains(&specifier) {
    let module_url =
      format!("{}node/{}.ts", NODE_COMPAT_URL.as_str(), specifier);
    Some(Url::parse(&module_url).unwrap())
  } else {
    None
  }
}

pub async fn check_if_should_use_esm_loader(
  js_runtime: &mut JsRuntime,
  main_module: &str,
) -> Result<bool, AnyError> {
  // Decide if we're running with Node ESM loader or CJS loader.
  let source_code = &format!(
    r#"(async function checkIfEsm(main) {{
      const {{ resolveMainPath, shouldUseESMLoader }} = await import("{}");
      const resolvedMain = resolveMainPath(main);
      const useESMLoader = shouldUseESMLoader(resolvedMain);
      return useESMLoader;
    }})('{}');"#,
    MODULE_URL_STR.as_str(),
    escape_for_single_quote_string(main_module),
  );
  let result =
    js_runtime.execute_script(&located_script_name!(), source_code)?;
  let use_esm_loader_global = js_runtime.resolve_value(result).await?;
  let use_esm_loader = {
    let scope = &mut js_runtime.handle_scope();
    let use_esm_loader_local = use_esm_loader_global.open(scope);
    use_esm_loader_local.boolean_value(scope)
  };

  Ok(use_esm_loader)
}

pub fn load_cjs_module(
  js_runtime: &mut JsRuntime,
  main_module: &str,
) -> Result<(), AnyError> {
  let source_code = &format!(
    r#"(async function loadCjsModule(main) {{
      const Module = await import("{}");
      Module.default._load(main, null, true);
    }})('{}');"#,
    MODULE_URL_STR.as_str(),
    escape_for_single_quote_string(main_module),
  );

  js_runtime.execute_script(&located_script_name!(), source_code)?;
  Ok(())
}

fn escape_for_single_quote_string(text: &str) -> String {
  text.replace(r"\", r"\\").replace("'", r"\'")
}
