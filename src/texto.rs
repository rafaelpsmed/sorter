//! Normalização de texto para comparar nomes de arquivo com palavras-chave.

/// Minúsculas, sem acentos, só letras e números separados por um espaço.
/// Também separa letras de números: "TC123_cranio" -> "tc 123 cranio".
pub fn normalizar(s: &str) -> String {
    let mut saida = String::with_capacity(s.len());
    let mut anterior: Option<char> = None;
    for c in s.chars().flat_map(char::to_lowercase).map(sem_acento) {
        if !c.is_alphanumeric() {
            saida.push(' ');
            anterior = None;
            continue;
        }
        if let Some(a) = anterior
            && a.is_numeric() != c.is_numeric()
        {
            saida.push(' ');
        }
        saida.push(c);
        anterior = Some(c);
    }
    saida.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Palavras curtas (até 3 letras, como "tc" ou "rm") precisam aparecer como
/// palavra inteira; palavras maiores podem aparecer dentro de outra
/// ("radiografia" casa com "radiografias").
pub fn contem_palavra(texto_normalizado: &str, palavra_normalizada: &str) -> bool {
    if palavra_normalizada.is_empty() {
        return false;
    }
    if palavra_normalizada.chars().count() <= 3 {
        format!(" {texto_normalizado} ").contains(&format!(" {palavra_normalizada} "))
    } else {
        texto_normalizado.contains(palavra_normalizada)
    }
}

fn sem_acento(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        outro => outro,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn normaliza() {
        assert_eq!(normalizar("Ressonância_Magnética-TC123.v2"), "ressonancia magnetica tc 123 v 2");
        assert_eq!(normalizar("  "), "");
    }

    #[test]
    fn palavras_curtas_sao_inteiras() {
        assert!(contem_palavra("laudo tc cranio", "tc"));
        assert!(!contem_palavra("etc e tal", "tc"));
        assert!(contem_palavra("raio x torax", "raio x"));
        assert!(contem_palavra("atlas de radiografias", "radiografia"));
    }
}
