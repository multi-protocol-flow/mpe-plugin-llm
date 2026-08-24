//! Binary entry point for MPE LLM plugin.

use mpe_plugin_llm::LlmPlugin;

fn install_rustls() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

fn main() {
    install_rustls();
    std::process::exit(mpe_plugin_sdk::__plugin_main::<LlmPlugin>());
}
