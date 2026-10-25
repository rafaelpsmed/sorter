//! Notificações na área de trabalho, via `notify-send` (libnotify).
//! Se o comando não existir, as notificações são simplesmente ignoradas.

use std::process::{Command, Stdio};

use crate::i18n::{tr, txt};

/// O que aconteceu com um item, para montar a notificação.
pub enum Resultado {
    Movido { nome: String, categoria: String },
    Erro { nome: String, erro: String },
}

/// Quantos itens aparecem listados; o resto vira "e mais N".
const MAXIMO_LISTADOS: usize = 5;

/// Uma notificação com tudo o que foi processado de uma vez, para não encher
/// a tela quando muitos itens chegam juntos.
pub fn notificar(resultados: &[Resultado]) {
    if let Some((titulo, corpo, urgente)) = resumir(resultados) {
        enviar(&titulo, &corpo, urgente);
    }
}

/// Notificação avulsa (ex.: config recarregado ou com erro).
pub fn avisar(titulo: &str, corpo: &str, urgente: bool) {
    // Só o corpo interpreta marcação; o título é exibido como texto puro.
    enviar(titulo, &escapar(corpo), urgente);
}

pub fn disponivel() -> bool {
    Command::new("notify-send")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn resumir(resultados: &[Resultado]) -> Option<(String, String, bool)> {
    if resultados.is_empty() {
        return None;
    }
    let erros = resultados.iter().filter(|r| matches!(r, Resultado::Erro { .. })).count();
    let movidos = resultados.len() - erros;

    let titulo = match (movidos, erros) {
        (1, 0) => txt("1 item organizado", "1 item organized").to_string(),
        (m, 0) => tr!("{m} itens organizados", "{m} items organized"),
        (0, 1) => txt("Erro ao mover 1 item", "Error moving 1 item").to_string(),
        (0, e) => tr!("Erro ao mover {e} itens", "Error moving {e} items"),
        (m, e) => tr!("{m} organizado(s), {e} com erro", "{m} organized, {e} failed"),
    };

    // Erros primeiro: são o que mais importa ver.
    let mut ordenados: Vec<&Resultado> = resultados.iter().filter(|r| matches!(r, Resultado::Erro { .. })).collect();
    ordenados.extend(resultados.iter().filter(|r| matches!(r, Resultado::Movido { .. })));
    let mut linhas: Vec<String> = ordenados
        .iter()
        .take(MAXIMO_LISTADOS)
        .map(|r| match r {
            Resultado::Movido { nome, categoria } => format!("{} → {}", escapar(nome), escapar(categoria)),
            Resultado::Erro { nome, erro } => format!("⚠ {}: {}", escapar(nome), escapar(erro)),
        })
        .collect();
    if resultados.len() > MAXIMO_LISTADOS {
        let resto = resultados.len() - MAXIMO_LISTADOS;
        linhas.push(tr!("e mais {resto}", "and {resto} more"));
    }
    Some((titulo, linhas.join("\n"), erros > 0))
}

fn enviar(titulo: &str, corpo: &str, urgente: bool) {
    let resultado = Command::new("notify-send")
        .args(["--app-name=Organizador PC", "--icon=folder"])
        .arg(format!("--urgency={}", if urgente { "critical" } else { "normal" }))
        .arg(titulo)
        .arg(corpo)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if let Err(e) = resultado {
        eprintln!(
            "{}",
            tr!("aviso: não foi possível enviar a notificação: {e}", "warning: could not send the notification: {e}")
        );
    }
}

/// O corpo da notificação aceita marcação simples (como HTML).
fn escapar(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod testes {
    use super::*;

    fn movido(nome: &str) -> Resultado {
        Resultado::Movido { nome: nome.into(), categoria: "VIDEOS".into() }
    }

    #[test]
    fn um_item() {
        let (titulo, corpo, urgente) = resumir(&[movido("filme.mkv")]).unwrap();
        assert_eq!(titulo, "1 item organizado");
        assert_eq!(corpo, "filme.mkv → VIDEOS");
        assert!(!urgente);
    }

    #[test]
    fn muitos_itens_com_erro_primeiro() {
        let mut r: Vec<Resultado> = (1..=6).map(|i| movido(&format!("v{i}.mp4"))).collect();
        r.push(Resultado::Erro { nome: "a&b.pdf".into(), erro: "sem permissão".into() });
        let (titulo, corpo, urgente) = resumir(&r).unwrap();
        assert_eq!(titulo, "6 organizado(s), 1 com erro");
        let linhas: Vec<&str> = corpo.lines().collect();
        assert_eq!(linhas[0], "⚠ a&amp;b.pdf: sem permissão");
        assert_eq!(linhas.len(), 6);
        assert_eq!(linhas[5], "e mais 2");
        assert!(urgente);
    }

    #[test]
    fn nada_para_notificar() {
        assert!(resumir(&[]).is_none());
    }
}
