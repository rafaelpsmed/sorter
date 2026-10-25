//! Move arquivos e pastas sem sobrescrever nada no destino.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use crate::i18n::txt;

/// Erro do Linux para rename entre sistemas de arquivos diferentes (EXDEV).
const EXDEV: i32 = 18;

/// Move `origem` (arquivo ou pasta) para dentro de `pasta_destino`, criando-a
/// se preciso, e devolve o caminho final. Se o nome já existir no destino, usa
/// "nome (1).ext", "nome (2).ext", ...
pub fn mover(origem: &Path, pasta_destino: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(pasta_destino)?;
    let nome = origem
        .file_name()
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, txt("caminho sem nome de arquivo", "path has no file name"))
        })?
        .to_string_lossy();
    let destino = caminho_livre(pasta_destino, &nome, origem.is_dir());

    match fs::rename(origem, &destino) {
        Ok(()) => Ok(destino),
        Err(e) if e.raw_os_error() == Some(EXDEV) => {
            copiar_e_apagar(origem, &destino)?;
            Ok(destino)
        }
        Err(e) => Err(e),
    }
}

/// Usado quando origem e destino estão em discos/partições diferentes.
/// Só apaga a origem depois que a cópia inteira deu certo.
fn copiar_e_apagar(origem: &Path, destino: &Path) -> io::Result<()> {
    let eh_pasta = fs::symlink_metadata(origem)?.is_dir();
    if let Err(e) = copiar(origem, destino) {
        let _ = if eh_pasta { fs::remove_dir_all(destino) } else { fs::remove_file(destino) };
        return Err(e);
    }
    if eh_pasta { fs::remove_dir_all(origem) } else { fs::remove_file(origem) }
}

fn copiar(origem: &Path, destino: &Path) -> io::Result<()> {
    let m = fs::symlink_metadata(origem)?;
    if m.is_symlink() {
        return std::os::unix::fs::symlink(fs::read_link(origem)?, destino);
    }
    if m.is_dir() {
        fs::create_dir(destino)?;
        for entrada in fs::read_dir(origem)? {
            let entrada = entrada?;
            copiar(&entrada.path(), &destino.join(entrada.file_name()))?;
        }
        fs::set_permissions(destino, m.permissions())?;
        return Ok(());
    }
    fs::copy(origem, destino)?;
    let f = File::options().write(true).open(destino)?;
    f.set_modified(m.modified()?)?;
    f.sync_all()
}

fn caminho_livre(pasta: &Path, nome: &str, eh_pasta: bool) -> PathBuf {
    let candidato = pasta.join(nome);
    if !existe(&candidato) {
        return candidato;
    }
    // Em pastas o ponto não indica extensão: "app-1.2.pkg" -> "app-1.2.pkg (1)".
    let (base, ext) = if eh_pasta { (nome, "") } else { separar_extensao(nome) };
    (1..).map(|n| pasta.join(format!("{base} ({n}){ext}"))).find(|c| !existe(c)).expect("sempre há um nome livre")
}

fn existe(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

/// "foto.jpg" -> ("foto", ".jpg"); "b.tar.gz" -> ("b", ".tar.gz"); "LEIAME" -> ("LEIAME", "")
fn separar_extensao(nome: &str) -> (&str, &str) {
    for dupla in [".tar.gz", ".tar.bz2", ".tar.xz", ".tar.zst"] {
        let corte = nome.len().saturating_sub(dupla.len());
        if corte > 0 && nome.is_char_boundary(corte) && nome[corte..].eq_ignore_ascii_case(dupla) {
            return nome.split_at(corte);
        }
    }
    match nome.rfind('.') {
        Some(i) if i > 0 => nome.split_at(i),
        _ => (nome, ""),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn separa_extensao() {
        assert_eq!(separar_extensao("foto.jpg"), ("foto", ".jpg"));
        assert_eq!(separar_extensao("b.TAR.GZ"), ("b", ".TAR.GZ"));
        assert_eq!(separar_extensao("LEIAME"), ("LEIAME", ""));
        assert_eq!(separar_extensao(".bashrc"), (".bashrc", ""));
    }

    #[test]
    fn nao_sobrescreve() {
        let dir = std::env::temp_dir().join(format!("organizador-teste-{}", std::process::id()));
        let origem = dir.join("origem");
        let destino = dir.join("destino");
        fs::create_dir_all(&origem).unwrap();
        fs::create_dir_all(&destino).unwrap();
        fs::write(destino.join("a.txt"), "antigo").unwrap();
        fs::write(origem.join("a.txt"), "novo").unwrap();

        let final_ = mover(&origem.join("a.txt"), &destino).unwrap();
        assert_eq!(final_, destino.join("a (1).txt"));
        assert_eq!(fs::read_to_string(destino.join("a.txt")).unwrap(), "antigo");
        assert_eq!(fs::read_to_string(&final_).unwrap(), "novo");
        assert!(!origem.join("a.txt").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn copia_pasta_recursivamente() {
        let dir = std::env::temp_dir().join(format!("organizador-copia-{}", std::process::id()));
        let origem = dir.join("projeto");
        fs::create_dir_all(origem.join("src/sub")).unwrap();
        fs::write(origem.join("src/sub/a.txt"), "a").unwrap();
        std::os::unix::fs::symlink("src/sub/a.txt", origem.join("link")).unwrap();

        copiar_e_apagar(&origem, &dir.join("copia")).unwrap();
        assert!(!origem.exists());
        assert_eq!(fs::read_to_string(dir.join("copia/src/sub/a.txt")).unwrap(), "a");
        assert_eq!(fs::read_link(dir.join("copia/link")).unwrap(), Path::new("src/sub/a.txt"));
        fs::remove_dir_all(&dir).unwrap();
    }
}
