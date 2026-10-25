//! Idioma das mensagens (português ou inglês), escolhido pelo ambiente:
//! `LC_ALL`, `LC_MESSAGES` ou `LANG` começando com "pt" → português; qualquer
//! outro valor → inglês. Nos testes, o idioma é sempre o português.

use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Idioma {
    Pt,
    En,
}

pub fn idioma() -> Idioma {
    static IDIOMA: OnceLock<Idioma> = OnceLock::new();
    *IDIOMA.get_or_init(|| {
        if cfg!(test) {
            return Idioma::Pt;
        }
        let valor = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|v| std::env::var(v).ok())
            .find(|v| !v.is_empty())
            .unwrap_or_default();
        if valor.to_lowercase().starts_with("pt") { Idioma::Pt } else { Idioma::En }
    })
}

/// Nome padrão da pasta para itens sem categoria.
pub fn nome_outros() -> &'static str {
    txt("OUTROS", "OTHERS")
}

/// Escolhe o texto no idioma atual.
pub fn txt(pt: &'static str, en: &'static str) -> &'static str {
    match idioma() {
        Idioma::Pt => pt,
        Idioma::En => en,
    }
}

/// Como `format!`, com um texto para cada idioma: `tr!("olá {x}", "hello {x}")`.
macro_rules! tr {
    ($pt:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        match $crate::i18n::idioma() {
            $crate::i18n::Idioma::Pt => format!($pt $(, $arg)*),
            $crate::i18n::Idioma::En => format!($en $(, $arg)*),
        }
    };
}
pub(crate) use tr;
