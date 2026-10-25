//! Leitura do arquivo de configuração (texto simples).
//!
//! Formato:
//!
//! ```text
//! PASTA_FONTE: ~/Downloads
//! PASTA_OUTROS: ~/Downloads/OUTROS      (opcional)
//! ESPERA_SEGUNDOS: 5                    (opcional)
//! NOTIFICACOES: sim                     (opcional)
//! CATEGORIAS:
//! 1. VIDEOS: ~/Videos | ext: mp4, mkv
//! 2. RADIOLOGIA: ~/Literatura/Radiologia | ext: pdf | palavras: tc, rm
//! ```
//!
//! Todas as chaves também existem em inglês: `SOURCE_DIR`, `OTHERS_DIR`,
//! `WAIT_SECONDS`, `NOTIFICATIONS`, `CATEGORIES` e, nas categorias,
//! `ext` / `words`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::i18n::{nome_outros, tr};
use crate::texto::normalizar;

#[derive(Debug, Clone)]
pub struct Categoria {
    pub nome: String,
    pub destino: PathBuf,
    /// Extensões em minúsculas, sem o ponto ("pdf", "tar.gz").
    pub extensoes: Vec<String>,
    /// Palavras-chave já normalizadas (minúsculas, sem acento).
    pub palavras: Vec<String>,
}

#[derive(Debug)]
pub struct Config {
    pub fonte: PathBuf,
    pub outros: PathBuf,
    pub espera_segundos: u64,
    pub notificacoes: bool,
    pub categorias: Vec<Categoria>,
    pub avisos: Vec<String>,
}

pub fn carregar(caminho: &Path) -> Result<Config, String> {
    let texto = fs::read_to_string(caminho)
        .map_err(|e| tr!("não foi possível ler {}: {e}", "could not read {}: {e}", caminho.display()))?;
    interpretar(&texto, &pasta_home())
}

pub fn pasta_home() -> PathBuf {
    env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

pub fn interpretar(texto: &str, home: &Path) -> Result<Config, String> {
    let mut fonte = None;
    let mut outros = None;
    let mut espera_segundos = 5;
    let mut notificacoes = true;
    let mut categorias = Vec::new();
    let mut em_categorias = false;

    for (i, linha) in texto.lines().enumerate() {
        let n = i + 1;
        let linha = linha.trim();
        if linha.is_empty() || linha.starts_with('#') {
            continue;
        }
        let Some((chave, valor)) = linha.split_once(':') else {
            return Err(tr!(
                "linha {n}: esperado 'CHAVE: valor', encontrado '{linha}'",
                "line {n}: expected 'KEY: value', found '{linha}'"
            ));
        };
        let valor = valor.trim();

        match normalizar_chave(chave).as_str() {
            "PASTA_FONTE" | "SOURCE_DIR" => {
                fonte = Some(resolver_caminho(valor, home).map_err(|e| erro_na_linha(n, &e))?)
            }
            "PASTA_OUTROS" | "OTHERS_DIR" => {
                outros = Some(resolver_caminho(valor, home).map_err(|e| erro_na_linha(n, &e))?)
            }
            "ESPERA_SEGUNDOS" | "WAIT_SECONDS" => {
                espera_segundos = valor.parse().map_err(|_| {
                    erro_na_linha(
                        n,
                        &tr!("ESPERA_SEGUNDOS deve ser um número inteiro", "WAIT_SECONDS must be a whole number"),
                    )
                })?;
            }
            "NOTIFICACOES" | "NOTIFICACAO" | "NOTIFICATIONS" | "NOTIFICATION" => {
                notificacoes = interpretar_sim_nao(valor).ok_or_else(|| {
                    erro_na_linha(
                        n,
                        &tr!("NOTIFICACOES deve ser 'sim' ou 'nao'", "NOTIFICATIONS must be 'yes' or 'no'"),
                    )
                })?;
            }
            "CATEGORIAS" | "CATEGORIES" => em_categorias = true,
            _ if em_categorias => {
                categorias.push(interpretar_categoria(linha, home).map_err(|e| erro_na_linha(n, &e))?);
            }
            _ => {
                return Err(tr!("linha {n}: chave desconhecida '{}'", "line {n}: unknown key '{}'", chave.trim()));
            }
        }
    }

    let fonte = fonte.ok_or_else(|| tr!("faltou a linha PASTA_FONTE", "the SOURCE_DIR line is missing"))?;
    if categorias.is_empty() {
        return Err(tr!(
            "nenhuma categoria definida (adicione linhas depois de CATEGORIAS:)",
            "no categories defined (add lines after CATEGORIES:)"
        ));
    }
    let outros = outros.unwrap_or_else(|| fonte.join(nome_outros()));

    let mut avisos = Vec::new();
    for c in &categorias {
        if c.destino == fonte {
            return Err(tr!(
                "a categoria {} aponta para a própria PASTA_FONTE",
                "category {} points to the source folder itself",
                c.nome
            ));
        }
        if c.extensoes.is_empty() && c.palavras.is_empty() {
            avisos.push(tr!(
                "a categoria {} não tem 'ext' nem 'palavras' e nunca receberá arquivos",
                "category {} has neither 'ext' nor 'words' and will never receive files",
                c.nome
            ));
        }
    }
    if outros == fonte {
        return Err(tr!(
            "PASTA_OUTROS não pode ser a própria PASTA_FONTE",
            "OTHERS_DIR cannot be the source folder itself"
        ));
    }

    Ok(Config { fonte, outros, espera_segundos, notificacoes, categorias, avisos })
}

fn erro_na_linha(n: usize, erro: &str) -> String {
    tr!("linha {n}: {erro}", "line {n}: {erro}")
}

/// `1. VIDEOS: ~/Videos | ext: mp4, mkv | palavras: aula, curso`
fn interpretar_categoria(linha: &str, home: &Path) -> Result<Categoria, String> {
    let mut partes = linha.split('|');
    let principal = partes.next().unwrap_or_default();
    let (nome, destino) = principal
        .split_once(':')
        .ok_or_else(|| tr!("categoria deve ter o formato 'NOME: pasta'", "category must look like 'NAME: folder'"))?;
    let nome = remover_numeracao(nome).trim().to_string();
    let destino = destino.trim();
    if nome.is_empty() || destino.is_empty() {
        return Err(tr!("categoria sem nome ou sem pasta de destino", "category without a name or destination folder"));
    }

    let mut extensoes = Vec::new();
    let mut palavras = Vec::new();
    for parte in partes {
        let parte = parte.trim();
        if parte.is_empty() {
            continue;
        }
        let (chave, lista) = parte.split_once(':').ok_or_else(|| {
            tr!(
                "esperado 'ext: ...' ou 'palavras: ...', encontrado '{parte}'",
                "expected 'ext: ...' or 'words: ...', found '{parte}'"
            )
        })?;
        let itens = lista.split(',').map(str::trim).filter(|s| !s.is_empty());
        match normalizar_chave(chave).as_str() {
            "EXT" | "EXTENSAO" | "EXTENSOES" | "EXTENSION" | "EXTENSIONS" => {
                extensoes.extend(itens.map(|e| e.trim_start_matches('.').to_lowercase()));
            }
            "PALAVRAS" | "PALAVRA" | "PALAVRAS_CHAVE" | "WORDS" | "WORD" | "KEYWORDS" => {
                palavras.extend(itens.map(normalizar).filter(|p| !p.is_empty()));
            }
            _ => {
                return Err(tr!(
                    "opção desconhecida '{}' (use 'ext' ou 'palavras')",
                    "unknown option '{}' (use 'ext' or 'words')",
                    chave.trim()
                ));
            }
        }
    }

    Ok(Categoria { nome, destino: resolver_caminho(destino, home)?, extensoes, palavras })
}

/// Aceita `~/x`, `$HOME/x`, `$XDG_VIDEOS_DIR/x` (pastas padrão do usuário,
/// que mudam de nome conforme o idioma do sistema), caminhos absolutos e
/// também `home/usuario/x` (sem a barra inicial, como no exemplo da spec).
/// Outros caminhos relativos são considerados relativos à pasta pessoal.
pub fn resolver_caminho(valor: &str, home: &Path) -> Result<PathBuf, String> {
    let valor = valor.trim();
    let (inicio, resto) = match valor.split_once('/') {
        Some((a, b)) => (a, Some(b)),
        None => (valor, None),
    };
    let base = if inicio == "~" || inicio == "$HOME" {
        home.to_path_buf()
    } else if let Some(nome) = inicio.strip_prefix("$XDG_").and_then(|v| v.strip_suffix("_DIR")) {
        pasta_do_usuario(nome, home)
            .ok_or_else(|| tr!("variável desconhecida {inicio}", "unknown variable {inicio}"))?
    } else if valor.starts_with('/') {
        return Ok(PathBuf::from(valor));
    } else if valor.starts_with("home/") {
        return Ok(Path::new("/").join(valor));
    } else {
        return Ok(home.join(valor));
    };
    Ok(match resto {
        Some(r) if !r.is_empty() => base.join(r),
        _ => base,
    })
}

/// Lê ~/.config/user-dirs.dirs (ex.: XDG_VIDEOS_DIR="$HOME/Vídeos"); se a
/// pasta não estiver lá, usa o nome padrão em inglês.
fn pasta_do_usuario(nome: &str, home: &Path) -> Option<PathBuf> {
    let chave = format!("XDG_{nome}_DIR=");
    let do_arquivo = fs::read_to_string(home.join(".config/user-dirs.dirs")).ok().and_then(|texto| {
        texto.lines().find_map(|l| {
            let v = l.trim().strip_prefix(&chave)?.trim_matches('"');
            Some(match v.strip_prefix("$HOME/") {
                Some(r) => home.join(r),
                None => PathBuf::from(v),
            })
        })
    });
    let padrao = match nome {
        "DESKTOP" => "Desktop",
        "DOWNLOAD" => "Downloads",
        "DOCUMENTS" => "Documents",
        "MUSIC" => "Music",
        "PICTURES" => "Pictures",
        "VIDEOS" => "Videos",
        "TEMPLATES" => "Templates",
        "PUBLICSHARE" => "Public",
        _ => return do_arquivo,
    };
    Some(do_arquivo.unwrap_or_else(|| home.join(padrao)))
}

/// "1. Palavras-Chave " -> "PALAVRAS_CHAVE"
fn normalizar_chave(chave: &str) -> String {
    normalizar(remover_numeracao(chave)).to_uppercase().replace(' ', "_")
}

/// Remove prefixos como "1.", "2)", "-" ou "*".
fn remover_numeracao(s: &str) -> &str {
    let s = s.trim_start();
    let sem_digitos = s.trim_start_matches(|c: char| c.is_ascii_digit());
    let s = if sem_digitos.len() != s.len() {
        sem_digitos.trim_start_matches(['.', ')'])
    } else {
        s.trim_start_matches(['-', '*'])
    };
    s.trim_start()
}

fn interpretar_sim_nao(valor: &str) -> Option<bool> {
    match normalizar(valor).as_str() {
        "sim" | "s" | "true" | "1" | "yes" => Some(true),
        "nao" | "n" | "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const HOME: &str = "/home/teste";

    #[test]
    fn le_exemplo_completo() {
        let texto = "\
# comentário
PASTA_FONTE: ~/Downloads
ESPERA_SEGUNDOS: 3
NOTIFICAÇÕES: não
CATEGORIAS:
1. VIDEOS: ~/Videos | ext: .MP4, mkv
2. LITERATURA: home/teste/Literatura | ext: pdf, epub
3. RADIOLOGIA: Literatura/Radiologia | ext: pdf | palavras: TC, Ressonância
";
        let cfg = interpretar(texto, Path::new(HOME)).unwrap();
        assert_eq!(cfg.fonte, PathBuf::from("/home/teste/Downloads"));
        assert_eq!(cfg.outros, PathBuf::from("/home/teste/Downloads/OUTROS"));
        assert_eq!(cfg.espera_segundos, 3);
        assert!(!cfg.notificacoes);
        assert_eq!(cfg.categorias.len(), 3);
        assert_eq!(cfg.categorias[0].nome, "VIDEOS");
        assert_eq!(cfg.categorias[0].extensoes, ["mp4", "mkv"]);
        assert_eq!(cfg.categorias[1].destino, PathBuf::from("/home/teste/Literatura"));
        assert_eq!(cfg.categorias[2].destino, PathBuf::from("/home/teste/Literatura/Radiologia"));
        assert_eq!(cfg.categorias[2].palavras, ["tc", "ressonancia"]);
        assert!(cfg.avisos.is_empty());
    }

    #[test]
    fn aceita_chaves_em_ingles() {
        let texto = "\
SOURCE_DIR: ~/Downloads
WAIT_SECONDS: 7
NOTIFICATIONS: no
CATEGORIES:
1. BOOKS: ~/Books | ext: pdf | words: novel, Ficção
";
        let cfg = interpretar(texto, Path::new(HOME)).unwrap();
        assert_eq!(cfg.espera_segundos, 7);
        assert!(!cfg.notificacoes);
        assert_eq!(cfg.categorias[0].extensoes, ["pdf"]);
        assert_eq!(cfg.categorias[0].palavras, ["novel", "ficcao"]);
    }

    #[test]
    fn resolve_pastas_do_usuario() {
        let home = std::env::temp_dir().join(format!("organizador-home-{}", std::process::id()));
        fs::create_dir_all(home.join(".config")).unwrap();
        fs::write(home.join(".config/user-dirs.dirs"), "XDG_VIDEOS_DIR=\"$HOME/Vídeos\"\n").unwrap();
        assert_eq!(resolver_caminho("$XDG_VIDEOS_DIR/Aulas", &home).unwrap(), home.join("Vídeos/Aulas"));
        assert_eq!(resolver_caminho("$XDG_MUSIC_DIR", &home).unwrap(), home.join("Music"));
        assert!(resolver_caminho("$XDG_INEXISTENTE_DIR", &home).is_err());
        assert_eq!(resolver_caminho("~", &home).unwrap(), home);
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn exige_pasta_fonte_e_categorias() {
        assert!(interpretar("CATEGORIAS:\n1. A: ~/a | ext: x", Path::new(HOME)).is_err());
        assert!(interpretar("PASTA_FONTE: ~/Downloads\nCATEGORIAS:", Path::new(HOME)).is_err());
    }

    #[test]
    fn avisa_categoria_sem_regras() {
        let cfg = interpretar("PASTA_FONTE: ~/D\nCATEGORIAS:\n1. VAZIA: ~/V", Path::new(HOME)).unwrap();
        assert_eq!(cfg.avisos.len(), 1);
    }

    #[test]
    fn rejeita_opcao_desconhecida() {
        let r = interpretar("PASTA_FONTE: ~/D\nCATEGORIAS:\n1. A: ~/a | tipo: x", Path::new(HOME));
        assert!(r.unwrap_err().contains("opção desconhecida"));
    }
}
