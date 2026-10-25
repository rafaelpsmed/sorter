mod classificador;
mod config;
mod i18n;
mod mover;
mod notificacao;
mod texto;

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use classificador::Decisao;

use notify::event::{AccessKind, AccessMode};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use config::Config;
use i18n::{nome_outros, tr, txt};
use notificacao::Resultado;

const AJUDA_PT: &str = "\
sorter — monitora uma pasta e move cada arquivo para a pasta da sua categoria

USO:
    sorter [opções]

OPÇÕES:
    -c, --config <arquivo>  arquivo de configuração a usar
        --uma-vez           organiza o que já existe na pasta e sai (--once)
        --simular           mostra o que seria feito, sem mover nada, e sai (--dry-run)
    -h, --help              mostra esta ajuda

Sem --config, procura nesta ordem:
    $XDG_CONFIG_HOME/sorter/config.txt  (normalmente ~/.config/sorter/config.txt)
    config.txt na mesma pasta do executável
";

const AJUDA_EN: &str = "\
sorter — watches a folder and moves each file to its category folder

USAGE:
    sorter [options]

OPTIONS:
    -c, --config <file>     configuration file to use
        --once              organize what is already in the folder, then exit (--uma-vez)
        --dry-run           show what would be done without moving anything, then exit (--simular)
    -h, --help              show this help

Without --config, it looks in this order:
    $XDG_CONFIG_HOME/sorter/config.txt  (usually ~/.config/sorter/config.txt)
    config.txt next to the executable
";

fn ajuda() -> &'static str {
    txt(AJUDA_PT, AJUDA_EN)
}

/// Extensões de arquivos que ainda estão sendo baixados.
const TEMPORARIOS: &[&str] = &["part", "crdownload", "download", "partial", "tmp", "opdownload", "!qb", "aria2"];

struct Opcoes {
    config: Option<PathBuf>,
    uma_vez: bool,
    simular: bool,
}

fn main() -> ExitCode {
    let opcoes = match ler_argumentos() {
        Ok(Some(o)) => o,
        Ok(None) => return ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}: {e}\n\n{}", txt("erro", "error"), ajuda());
            return ExitCode::from(2);
        }
    };

    let Some(caminho_config) = opcoes.config.clone().or_else(procurar_config) else {
        eprintln!(
            "{}\n\n{}",
            txt("erro: arquivo de configuração não encontrado.", "error: configuration file not found."),
            ajuda()
        );
        return ExitCode::FAILURE;
    };
    let monitorando = !(opcoes.simular || opcoes.uma_vez);
    let notify_send = monitorando && notificacao::disponivel();
    if monitorando && !notify_send {
        eprintln!(
            "{}",
            txt(
                "aviso: notify-send não encontrado; as notificações ficam desligadas",
                "warning: notify-send not found; notifications are disabled"
            )
        );
    }

    let cfg = match config::carregar(&caminho_config) {
        Ok(c) => c,
        // Como serviço, sair faria o systemd reiniciar o programa em loop;
        // em vez disso, espera o config ser corrigido.
        Err(e) if monitorando => {
            relatar_erro_config(&e, txt("corrija e salve o arquivo", "fix and save the file"), notify_send);
            esperar_config_valida(&caminho_config, notify_send)
        }
        Err(e) => {
            eprintln!("{}", tr!("erro no config {}: {e}", "config error in {}: {e}", caminho_config.display()));
            return ExitCode::FAILURE;
        }
    };

    let mut org = Organizador::novo(cfg, &caminho_config, opcoes.simular, notify_send);
    if let Err(e) = org.preparar_pastas() {
        eprintln!("{}: {e}", txt("erro", "error"));
        return ExitCode::FAILURE;
    }

    if !monitorando {
        let mut analisados = org.reclassificar_outros().len();
        let itens = org.itens_da_fonte();
        for item in &itens {
            org.processar(item);
        }
        analisados += itens.len();
        println!("{}", tr!("{analisados} item(ns) analisado(s).", "{analisados} item(s) checked."));
        return ExitCode::SUCCESS;
    }

    match org.monitorar() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}: {e}", txt("erro", "error"));
            ExitCode::FAILURE
        }
    }
}

fn ler_argumentos() -> Result<Option<Opcoes>, String> {
    let mut opcoes = Opcoes { config: None, uma_vez: false, simular: false };
    let mut args = env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("-c" | "--config") => {
                let valor =
                    args.next().ok_or_else(|| txt("--config precisa de um caminho", "--config needs a path"))?;
                opcoes.config = Some(PathBuf::from(valor));
            }
            Some("--uma-vez" | "--once") => opcoes.uma_vez = true,
            Some("--simular" | "--dry-run") => opcoes.simular = true,
            Some("-h" | "--help") => {
                print!("{}", ajuda());
                return Ok(None);
            }
            _ => return Err(tr!("opção desconhecida: {}", "unknown option: {}", arg.to_string_lossy())),
        }
    }
    Ok(Some(opcoes))
}

fn procurar_config() -> Option<PathBuf> {
    let base =
        env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| config::pasta_home().join(".config"));
    let mut candidatos = vec![base.join("sorter").join("config.txt")];
    if let Some(dir) = env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        candidatos.push(dir.join("config.txt"));
    }
    candidatos.into_iter().find(|c| c.is_file())
}

fn relatar_erro_config(erro: &str, o_que_fazer: &str, notify_send: bool) {
    eprintln!("{}", tr!("erro no config ({o_que_fazer}): {erro}", "config error ({o_que_fazer}): {erro}"));
    if notify_send {
        notificacao::avisar(&tr!("Erro no config — {o_que_fazer}", "Config error — {o_que_fazer}"), erro, true);
    }
}

/// Usado só quando o programa inicia com um config inválido: confere o
/// arquivo a cada 2 segundos até ele ser salvo de um jeito válido.
fn esperar_config_valida(caminho: &Path, notify_send: bool) -> Config {
    let mut ultimo = fs::read_to_string(caminho).ok();
    loop {
        thread::sleep(Duration::from_secs(2));
        let atual = fs::read_to_string(caminho).ok();
        if atual == ultimo {
            continue;
        }
        ultimo = atual;
        let Some(texto) = &ultimo else { continue };
        match config::interpretar(texto, &config::pasta_home()) {
            Ok(cfg) => {
                println!("{}", txt("config corrigido, iniciando", "config fixed, starting"));
                return cfg;
            }
            Err(e) => relatar_erro_config(&e, txt("corrija e salve o arquivo", "fix and save the file"), notify_send),
        }
    }
}

/// Resumo de um item usado para saber se ele parou de mudar.
#[derive(PartialEq)]
struct Assinatura {
    arquivos: u64,
    tamanho: u64,
    modificado: Option<SystemTime>,
    tem_temporario: bool,
}

struct Pendente {
    assinatura: Assinatura,
    desde: Instant,
}

/// Cada item no topo da pasta monitorada (um arquivo ou uma subpasta
/// inteira) é tratado como uma unidade: subpastas nunca são desmontadas.
struct Organizador {
    cfg: Config,
    /// Pastas dentro da pasta monitorada que não devem ser organizadas
    /// (destinos de categorias e a pasta OUTROS, quando ficam lá dentro).
    excluidas: Vec<PathBuf>,
    caminho_config: PathBuf,
    simular: bool,
    notify_send: bool,
}

impl Organizador {
    fn novo(cfg: Config, caminho_config: &Path, simular: bool, notify_send: bool) -> Self {
        let caminho_config = fs::canonicalize(caminho_config).unwrap_or_else(|_| caminho_config.to_path_buf());
        let excluidas = excluidas(&cfg);
        Organizador { cfg, excluidas, caminho_config, simular, notify_send }
    }

    fn preparar_pastas(&self) -> Result<(), String> {
        for aviso in &self.cfg.avisos {
            eprintln!("{}: {aviso}", txt("aviso", "warning"));
        }
        if self.simular {
            return Ok(());
        }
        let pastas =
            [&self.cfg.fonte, &self.cfg.outros].into_iter().chain(self.cfg.categorias.iter().map(|c| &c.destino));
        for p in pastas {
            if !p.is_dir() {
                fs::create_dir_all(p)
                    .map_err(|e| tr!("não foi possível criar {}: {e}", "could not create {}: {e}", p.display()))?;
                println!("{}", tr!("pasta criada: {}", "folder created: {}", p.display()));
            }
        }
        Ok(())
    }

    fn notificar(&self, resultados: &[Resultado]) {
        if self.notify_send && self.cfg.notificacoes {
            notificacao::notificar(resultados);
        }
    }

    fn monitorar(&mut self) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(tx)
            .map_err(|e| tr!("falha ao iniciar o monitor: {e}", "failed to start the watcher: {e}"))?;
        self.vigiar_fonte(&mut watcher)?;
        // Muitos editores salvam gravando um arquivo novo e trocando pelo
        // antigo, então vigiamos a pasta do config, não o arquivo em si.
        if let Some(pasta) = self.caminho_config.parent()
            && !pasta.starts_with(&self.cfg.fonte)
        {
            watcher
                .watch(pasta, RecursiveMode::NonRecursive)
                .map_err(|e| tr!("falha ao monitorar {}: {e}", "failed to watch {}: {e}", pasta.display()))?;
        }

        let mut texto_config = fs::read_to_string(&self.caminho_config).ok();
        let mut pendentes = HashMap::new();
        // O monitor já está ativo, então nada que chegue durante a varredura inicial se perde.
        self.varrer(&mut pendentes);

        let intervalo = Duration::from_millis(500);
        let mut ultima_verificacao = Instant::now();
        let mut config_mudou: Option<Instant> = None;
        loop {
            match rx.recv_timeout(intervalo) {
                Ok(Ok(evento)) if evento_relevante(&evento.kind) => {
                    for p in &evento.paths {
                        if self.eh_o_config(p) {
                            config_mudou = Some(Instant::now());
                        } else if let Some(item) = self.item_de(p) {
                            self.agendar(item, &mut pendentes);
                        }
                    }
                }
                Ok(Ok(_)) => {}
                Ok(Err(e)) => eprintln!("{}", tr!("aviso do monitor: {e}", "watcher warning: {e}")),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(txt("o monitor parou", "the watcher stopped").into());
                }
            }
            // Espera 1s de calma: editores costumam gravar em mais de um passo.
            if config_mudou.is_some_and(|t| t.elapsed() >= Duration::from_secs(1)) {
                config_mudou = None;
                self.recarregar(&mut watcher, &mut texto_config, &mut pendentes);
            }
            if ultima_verificacao.elapsed() >= intervalo {
                self.verificar(&mut pendentes);
                ultima_verificacao = Instant::now();
            }
        }
    }

    fn vigiar_fonte(&self, watcher: &mut RecommendedWatcher) -> Result<(), String> {
        watcher
            .watch(&self.cfg.fonte, RecursiveMode::Recursive)
            .map_err(|e| tr!("falha ao monitorar {}: {e}", "failed to watch {}: {e}", self.cfg.fonte.display()))?;
        println!(
            "{}",
            tr!(
                "monitorando {} ({} categorias, espera de {}s)",
                "watching {} ({} categories, {}s wait)",
                self.cfg.fonte.display(),
                self.cfg.categorias.len(),
                self.cfg.espera_segundos
            )
        );
        Ok(())
    }

    /// Reorganiza a pasta OUTROS com as regras atuais e agenda tudo o que
    /// está na pasta monitorada.
    fn varrer(&self, pendentes: &mut HashMap<PathBuf, Pendente>) {
        let resultados = self.reclassificar_outros();
        self.notificar(&resultados);
        for item in self.itens_da_fonte() {
            self.agendar(item, pendentes);
        }
    }

    fn eh_o_config(&self, p: &Path) -> bool {
        p.file_name() == self.caminho_config.file_name()
            && p.parent().and_then(|d| fs::canonicalize(d).ok()).as_deref() == self.caminho_config.parent()
    }

    /// O config foi salvo: aplica a nova versão. Se ela tiver erro, mantém a
    /// configuração anterior e avisa.
    fn recarregar(
        &mut self,
        watcher: &mut RecommendedWatcher,
        texto_config: &mut Option<String>,
        pendentes: &mut HashMap<PathBuf, Pendente>,
    ) {
        // Falha na leitura: o arquivo está no meio de uma troca; outro evento virá.
        let Ok(texto) = fs::read_to_string(&self.caminho_config) else { return };
        if texto_config.as_deref() == Some(texto.as_str()) {
            return; // salvo sem mudanças
        }
        let novo = config::interpretar(&texto, &config::pasta_home());
        *texto_config = Some(texto);
        let novo = match novo {
            Ok(c) => c,
            Err(e) => {
                relatar_erro_config(
                    &e,
                    txt("mantida a configuração anterior", "keeping the previous configuration"),
                    self.notify_send,
                );
                return;
            }
        };

        let fonte_antiga = std::mem::replace(&mut self.cfg, novo).fonte;
        self.excluidas = excluidas(&self.cfg);
        println!("{}", txt("config recarregado", "config reloaded"));
        if let Err(e) = self.preparar_pastas() {
            relatar_erro_config(&e, txt("pastas", "folders"), self.notify_send);
        }
        if self.cfg.fonte != fonte_antiga {
            let _ = watcher.unwatch(&fonte_antiga);
            if let Err(e) = self.vigiar_fonte(watcher) {
                relatar_erro_config(&e, txt("pasta monitorada", "watched folder"), self.notify_send);
            }
        }
        if self.notify_send && self.cfg.notificacoes {
            let corpo = tr!(
                "{} categorias · monitorando {}",
                "{} categories · watching {}",
                self.cfg.categorias.len(),
                self.cfg.fonte.display()
            );
            notificacao::avisar(txt("Configuração recarregada", "Configuration reloaded"), &corpo, false);
        }
        pendentes.clear();
        self.varrer(pendentes);
    }

    /// Itens no topo da pasta monitorada que devem ser organizados.
    fn itens_da_fonte(&self) -> Vec<PathBuf> {
        let Ok(entradas) = fs::read_dir(&self.cfg.fonte) else { return Vec::new() };
        let mut itens: Vec<PathBuf> =
            entradas.flatten().map(|e| e.path()).filter(|p| self.item_de(p).is_some()).collect();
        itens.sort();
        itens
    }

    /// Converte qualquer caminho dentro da fonte no item do topo a que ele
    /// pertence (ex.: fonte/pasta/sub/x.pdf -> fonte/pasta), ou None se o
    /// item deve ser ignorado.
    fn item_de(&self, p: &Path) -> Option<PathBuf> {
        let primeiro = p.strip_prefix(&self.cfg.fonte).ok()?.iter().next()?;
        let item = self.cfg.fonte.join(primeiro);
        let nome = primeiro.to_string_lossy();
        let ignorar = nome.starts_with('.')
            || eh_temporario(&nome)
            || item == self.caminho_config
            // destinos dentro da fonte, ou pastas que contêm um destino
            || self.excluidas.iter().any(|e| e.starts_with(&item));
        if ignorar {
            return None;
        }
        match fs::symlink_metadata(&item) {
            Ok(m) if m.is_file() || m.is_dir() => Some(item),
            _ => None, // links simbólicos, arquivos especiais ou item que já saiu
        }
    }

    fn agendar(&self, item: PathBuf, pendentes: &mut HashMap<PathBuf, Pendente>) {
        match pendentes.get_mut(&item) {
            // Já aguardando: só reinicia a espera (calcular a assinatura de
            // uma pasta grande a cada evento seria caro).
            Some(p) => p.desde = Instant::now(),
            None => {
                if let Some(assinatura) = assinar(&item) {
                    pendentes.insert(item, Pendente { assinatura, desde: Instant::now() });
                }
            }
        }
    }

    /// Processa os itens que ficaram ESPERA_SEGUNDOS sem mudar, ou seja, que
    /// já terminaram de ser copiados, baixados ou extraídos.
    fn verificar(&self, pendentes: &mut HashMap<PathBuf, Pendente>) {
        let espera = Duration::from_secs(self.cfg.espera_segundos);
        let vencidos: Vec<PathBuf> =
            pendentes.iter().filter(|(_, p)| p.desde.elapsed() >= espera).map(|(c, _)| c.clone()).collect();

        let mut resultados = Vec::new();
        for item in vencidos {
            let Some(atual) = assinar(&item) else {
                pendentes.remove(&item);
                continue;
            };
            let pendente = pendentes.get_mut(&item).expect("item veio do mapa");
            if atual != pendente.assinatura || atual.tem_temporario || atual.arquivos == 0 {
                // Pastas vazias também esperam: costumam ser downloads começando.
                pendente.assinatura = atual;
                pendente.desde = Instant::now();
                continue;
            }
            pendentes.remove(&item);
            resultados.extend(self.processar(&item));
        }
        self.notificar(&resultados);
    }

    /// Move um item da pasta monitorada para a sua categoria (ou OUTROS).
    fn processar(&self, item: &Path) -> Option<Resultado> {
        let decisao = self.decidir(item, &self.cfg.fonte)?;
        let (nome_categoria, destino) = match decisao.categoria {
            Some(c) => (c.nome.as_str(), &c.destino),
            None => (nome_outros(), &self.cfg.outros),
        };
        self.mover_item(item, nome_categoria, destino, &decisao.motivo)
    }

    /// Tira da pasta OUTROS o que, com as regras atuais, já tem categoria
    /// (ex.: uma categoria nova foi criada no config). O resto fica lá.
    fn reclassificar_outros(&self) -> Vec<Resultado> {
        let Ok(entradas) = fs::read_dir(&self.cfg.outros) else { return Vec::new() };
        let mut itens: Vec<PathBuf> = entradas
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let nome = p.file_name().unwrap_or_default().to_string_lossy();
                !nome.starts_with('.')
                    && !eh_temporario(&nome)
                    && fs::symlink_metadata(p).is_ok_and(|m| m.is_file() || m.is_dir())
            })
            .collect();
        itens.sort();
        itens
            .iter()
            .filter_map(|item| {
                let decisao = self.decidir(item, &self.cfg.outros)?;
                let c = decisao.categoria.filter(|c| c.destino != self.cfg.outros)?;
                self.mover_item(item, &c.nome, &c.destino, &decisao.motivo)
            })
            .collect()
    }

    /// `base`: a pasta onde o item está (os nomes de pastas abaixo dela
    /// contam como palavras-chave).
    fn decidir(&self, item: &Path, base: &Path) -> Option<Decisao<'_>> {
        if item.is_dir() {
            self.classificar_pasta(item, base)
        } else {
            let nome = item.file_name().unwrap_or_default().to_string_lossy();
            Some(classificar_arquivo(&self.cfg, item, &nome, &[]))
        }
    }

    fn mover_item(&self, item: &Path, nome_categoria: &str, destino: &Path, motivo: &str) -> Option<Resultado> {
        if self.simular {
            println!(
                "[{}] {} -> {} ({nome_categoria}: {motivo})",
                txt("simulação", "dry run"),
                item.display(),
                destino.display()
            );
            return None;
        }
        let nome = item.file_name().unwrap_or_default().to_string_lossy().into_owned();
        match mover::mover(item, destino) {
            Ok(final_) => {
                println!("{} -> {} ({nome_categoria}: {motivo})", item.display(), final_.display());
                Some(Resultado::Movido { nome, categoria: nome_categoria.to_string() })
            }
            Err(e) => {
                eprintln!("{}", tr!("erro ao mover {}: {e}", "error moving {}: {e}", item.display()));
                Some(Resultado::Erro { nome, erro: e.to_string() })
            }
        }
    }

    /// Classifica cada arquivo da pasta (o nome da pasta e das subpastas conta
    /// como palavra-chave) e a pasta inteira vai para a categoria que tiver
    /// mais da metade dos arquivos. Sem maioria, vai para OUTROS.
    fn classificar_pasta(&self, pasta: &Path, base: &Path) -> Option<Decisao<'_>> {
        let mut arquivos = Vec::new();
        listar_arquivos(pasta, &mut arquivos);
        if arquivos.is_empty() {
            return None;
        }
        let mut votos = vec![0usize; self.cfg.categorias.len()];
        for a in &arquivos {
            let relativo = a.strip_prefix(base).unwrap_or(a);
            let nome = relativo.file_name().unwrap_or_default().to_string_lossy();
            let pastas: Vec<String> = relativo
                .parent()
                .map(|r| r.iter().map(|c| c.to_string_lossy().into_owned()).collect())
                .unwrap_or_default();
            let pastas: Vec<&str> = pastas.iter().map(String::as_str).collect();
            if let Some(c) = classificar_arquivo(&self.cfg, a, &nome, &pastas).categoria {
                let i = self.cfg.categorias.iter().position(|x| std::ptr::eq(x, c)).expect("categoria do config");
                votos[i] += 1;
            }
        }

        let total = arquivos.len();
        let (indice, &maior) = votos.iter().enumerate().max_by_key(|&(i, v)| (*v, std::cmp::Reverse(i)))?;
        let categoria = &self.cfg.categorias[indice];
        if maior * 2 > total {
            Some(Decisao {
                categoria: Some(categoria),
                motivo: tr!(
                    "pasta: {maior} de {total} arquivos são {}",
                    "folder: {maior} of {total} files are {}",
                    categoria.nome
                ),
            })
        } else {
            Some(Decisao {
                categoria: None,
                motivo: if maior == 0 {
                    tr!(
                        "pasta: nenhum dos {total} arquivos casou com uma regra",
                        "folder: none of the {total} files matched a rule"
                    )
                } else {
                    tr!(
                        "pasta sem maioria: o máximo foi {maior} de {total} arquivos em {}",
                        "folder without a majority: the most was {maior} of {total} files in {}",
                        categoria.nome
                    )
                },
            })
        }
    }
}

/// Destinos (e a pasta OUTROS) que ficam dentro da pasta monitorada.
fn excluidas(cfg: &Config) -> Vec<PathBuf> {
    cfg.categorias
        .iter()
        .map(|c| c.destino.clone())
        .chain([cfg.outros.clone()])
        .filter(|d| d.starts_with(&cfg.fonte))
        .collect()
}

fn classificar_arquivo<'a>(cfg: &'a Config, caminho: &Path, nome: &str, pastas: &[&str]) -> Decisao<'a> {
    classificador::classificar(&cfg.categorias, nome, pastas, || {
        infer::get_from_path(caminho).ok().flatten().map(|t| t.extension().to_string())
    })
}

/// Arquivos comuns dentro de `pasta`, recursivamente, ignorando ocultos.
fn listar_arquivos(pasta: &Path, saida: &mut Vec<PathBuf>) {
    let Ok(entradas) = fs::read_dir(pasta) else { return };
    for entrada in entradas.flatten() {
        if entrada.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        match entrada.file_type() {
            Ok(t) if t.is_dir() => listar_arquivos(&entrada.path(), saida),
            Ok(t) if t.is_file() => saida.push(entrada.path()),
            _ => {}
        }
    }
}

fn assinar(item: &Path) -> Option<Assinatura> {
    let m = fs::symlink_metadata(item).ok()?;
    if m.is_file() {
        let nome = item.as_os_str().to_string_lossy();
        // Navegadores criam "x.pdf" vazio e baixam em "x.pdf.part" (Firefox)
        // ou similar; enquanto o temporário existir, o download não acabou.
        let tem_temporario = TEMPORARIOS.iter().any(|t| Path::new(&format!("{nome}.{t}")).exists());
        return Some(Assinatura { arquivos: 1, tamanho: m.len(), modificado: m.modified().ok(), tem_temporario });
    }
    let mut arquivos = Vec::new();
    listar_arquivos(item, &mut arquivos);
    let mut a = Assinatura { arquivos: 0, tamanho: 0, modificado: m.modified().ok(), tem_temporario: false };
    for arquivo in &arquivos {
        let Ok(m) = fs::metadata(arquivo) else { continue };
        a.arquivos += 1;
        a.tamanho += m.len();
        a.modificado = a.modificado.max(m.modified().ok());
        a.tem_temporario |= eh_temporario(&arquivo.file_name().unwrap_or_default().to_string_lossy());
    }
    Some(a)
}

fn eh_temporario(nome: &str) -> bool {
    let nome = nome.to_lowercase();
    TEMPORARIOS.iter().any(|t| nome.ends_with(&format!(".{t}")))
}

fn evento_relevante(tipo: &EventKind) -> bool {
    matches!(
        tipo,
        EventKind::Create(_)
            | EventKind::Modify(_)
            | EventKind::Access(AccessKind::Close(AccessMode::Write))
            | EventKind::Any
            | EventKind::Other
    )
}
