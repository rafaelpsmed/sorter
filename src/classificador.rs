//! Decide a categoria de um arquivo usando apenas regras locais.
//!
//! Cada categoria pode ter extensões, palavras-chave ou as duas coisas:
//!
//! | regras da categoria  | quando casa                         | prioridade |
//! |----------------------|-------------------------------------|------------|
//! | ext + palavras       | extensão E pelo menos uma palavra   | 3 (maior)  |
//! | só palavras          | pelo menos uma palavra              | 2          |
//! | só ext               | extensão                            | 1          |
//!
//! A categoria com maior prioridade vence; empate fica com a que tiver mais
//! palavras encontradas e, depois, com a que vem primeiro no config.
//!
//! As palavras são procuradas no nome do arquivo e nos nomes das subpastas
//! (dentro da pasta monitorada) onde ele está. Se a extensão do arquivo não
//! estiver em nenhuma categoria (ou não existir), usa-se o tipo detectado
//! pelos primeiros bytes do conteúdo.

use crate::config::Categoria;
use crate::i18n::{tr, txt};
use crate::texto::{contem_palavra, normalizar};

#[derive(Debug)]
pub struct Decisao<'a> {
    pub categoria: Option<&'a Categoria>,
    pub motivo: String,
}

/// `nome_arquivo`: nome com extensão. `pastas`: subpastas entre a pasta
/// monitorada e o arquivo. `tipo_detectado`: extensão sugerida pelo conteúdo.
pub fn classificar<'a>(
    categorias: &'a [Categoria],
    nome_arquivo: &str,
    pastas: &[&str],
    tipo_detectado: impl FnOnce() -> Option<String>,
) -> Decisao<'a> {
    let mut extensoes = extensoes_do_nome(nome_arquivo);
    let conhecida = extensoes.iter().any(|e| categorias.iter().any(|c| c.extensoes.contains(e)));
    let mut usou_conteudo = None;
    if !conhecida && let Some(tipo) = tipo_detectado() {
        usou_conteudo = Some(tipo.clone());
        extensoes.push(tipo);
    }

    let mut texto = remover_extensao(nome_arquivo).to_string();
    for p in pastas {
        texto.push(' ');
        texto.push_str(p);
    }
    let texto = normalizar(&texto);

    let mut melhor: Option<(&Categoria, u8, Vec<&str>, Option<&str>)> = None;
    for c in categorias {
        let ext = extensoes.iter().find(|e| c.extensoes.contains(e)).map(String::as_str);
        let achadas: Vec<&str> = c.palavras.iter().filter(|p| contem_palavra(&texto, p)).map(String::as_str).collect();
        let prioridade = match (c.extensoes.is_empty(), c.palavras.is_empty()) {
            (false, false) if ext.is_some() && !achadas.is_empty() => 3,
            (true, false) if !achadas.is_empty() => 2,
            (false, true) if ext.is_some() => 1,
            _ => 0,
        };
        if prioridade == 0 {
            continue;
        }
        let supera = match &melhor {
            None => true,
            Some((_, p, a, _)) => (prioridade, achadas.len()) > (*p, a.len()),
        };
        if supera {
            melhor = Some((c, prioridade, achadas, ext));
        }
    }

    match melhor {
        Some((c, _, achadas, ext)) => {
            let mut motivos = Vec::new();
            if let Some(e) = ext {
                if usou_conteudo.as_deref() == Some(e) {
                    motivos.push(tr!("conteúdo parece .{e}", "content looks like .{e}"));
                } else {
                    motivos.push(tr!("extensão .{e}", "extension .{e}"));
                }
            }
            if !achadas.is_empty() {
                motivos.push(tr!("palavras: {}", "words: {}", achadas.join(", ")));
            }
            Decisao { categoria: Some(c), motivo: motivos.join("; ") }
        }
        None => Decisao { categoria: None, motivo: txt("nenhuma regra casou", "no rule matched").into() },
    }
}

/// "Foto.JPG" -> ["jpg"]; "backup.tar.gz" -> ["tar.gz", "gz"]; ".bashrc" -> [].
/// A dupla vem primeiro para ter preferência.
pub fn extensoes_do_nome(nome: &str) -> Vec<String> {
    let nome = nome.to_lowercase();
    let partes: Vec<&str> = nome.trim_start_matches('.').split('.').collect();
    let n = partes.len();
    let mut v = Vec::new();
    if n >= 2 && !partes[n - 1].is_empty() {
        if n >= 3 && !partes[n - 2].is_empty() {
            v.push(format!("{}.{}", partes[n - 2], partes[n - 1]));
        }
        v.push(partes[n - 1].to_string());
    }
    v
}

fn remover_extensao(nome: &str) -> &str {
    match nome.rfind('.') {
        Some(i) if i > 0 => &nome[..i],
        _ => nome,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::path::PathBuf;

    fn cat(nome: &str, ext: &[&str], palavras: &[&str]) -> Categoria {
        Categoria {
            nome: nome.into(),
            destino: PathBuf::from("/x").join(nome),
            extensoes: ext.iter().map(|s| s.to_string()).collect(),
            palavras: palavras.iter().map(|s| normalizar(s)).collect(),
        }
    }

    fn categorias() -> Vec<Categoria> {
        vec![
            cat("VIDEOS", &["mp4", "mkv"], &[]),
            cat("LITERATURA", &["pdf", "epub"], &[]),
            cat("RADIOLOGIA", &["pdf"], &["tc", "ressonância", "radiografia"]),
            cat("AULAS", &[], &["aula"]),
            cat("COMPACTADOS", &["zip", "tar.gz"], &[]),
        ]
    }

    fn nome_da<'a>(d: &Decisao<'a>) -> Option<&'a str> {
        d.categoria.map(|c| c.nome.as_str())
    }

    #[test]
    fn por_extensao() {
        let cs = categorias();
        assert_eq!(nome_da(&classificar(&cs, "Filme.MKV", &[], || None)), Some("VIDEOS"));
        assert_eq!(nome_da(&classificar(&cs, "livro.pdf", &[], || None)), Some("LITERATURA"));
        assert_eq!(nome_da(&classificar(&cs, "b.tar.gz", &[], || None)), Some("COMPACTADOS"));
    }

    #[test]
    fn extensao_mais_palavra_vence_so_extensao() {
        let cs = categorias();
        let d = classificar(&cs, "TC_cranio_2024.pdf", &[], || None);
        assert_eq!(nome_da(&d), Some("RADIOLOGIA"));
        assert_eq!(d.motivo, "extensão .pdf; palavras: tc");
        assert_eq!(nome_da(&classificar(&cs, "Ressonancia joelho.pdf", &[], || None)), Some("RADIOLOGIA"));
    }

    #[test]
    fn palavra_na_subpasta() {
        let cs = categorias();
        let d = classificar(&cs, "capitulo1.pdf", &["Radiografias do tórax"], || None);
        assert_eq!(nome_da(&d), Some("RADIOLOGIA"));
    }

    #[test]
    fn so_palavras_aceita_qualquer_tipo() {
        let cs = categorias();
        assert_eq!(nome_da(&classificar(&cs, "aula 3.pptx", &[], || None)), Some("AULAS"));
        // ext+palavras (3) vence só palavras (2)
        assert_eq!(nome_da(&classificar(&cs, "aula tc.pdf", &[], || None)), Some("RADIOLOGIA"));
    }

    #[test]
    fn usa_conteudo_quando_extensao_desconhecida() {
        let cs = categorias();
        let d = classificar(&cs, "documento_sem_extensao", &[], || Some("pdf".into()));
        assert_eq!(nome_da(&d), Some("LITERATURA"));
        assert_eq!(d.motivo, "conteúdo parece .pdf");
        // extensão conhecida: não consulta o conteúdo
        let d = classificar(&cs, "x.mp4", &[], || panic!("não deveria ler o conteúdo"));
        assert_eq!(nome_da(&d), Some("VIDEOS"));
    }

    #[test]
    fn sem_categoria() {
        let cs = categorias();
        assert!(classificar(&cs, "planilha.xlsx", &[], || None).categoria.is_none());
    }

    #[test]
    fn extrai_extensoes() {
        assert_eq!(extensoes_do_nome("a.JPG"), ["jpg"]);
        assert_eq!(extensoes_do_nome("a.tar.gz"), ["tar.gz", "gz"]);
        assert!(extensoes_do_nome(".bashrc").is_empty());
        assert!(extensoes_do_nome("LEIAME").is_empty());
    }
}
